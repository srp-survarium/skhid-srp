use super::*;
use crate::v026g::MemoryGameRepository;
use crate::v026g::account::QuestInstance;
use crate::v026g::quests::QUEST_MESSAGE;
use survarium::v026g::lobby::client_model::{Client, Decoded};
use survarium::v026g::match_msgs::snapshot;
use survarium::v026g::quests::Quests;

const NOW: u64 = 1_000_000;
const MATCH: u32 = 0x77;

/// The original's pay: the rates of a match's experience, silver and
/// reputation at 1, 100 % and 100 %, with the game's wear and the play time
/// bonus to the drop chance (both off in srp's own terms). Every match
/// moves the balance rating, whatever its size (srp's own terms: from four
/// players).
fn terms() -> Terms {
    Terms {
        now: NOW,
        experience_rate: 1,
        money_rate: pay::MoneyRate::Flat(100),
        reputation_rate: 100,
        score_weights: SCORE_WEIGHTS,
        wear: true,
        playtime_bonus: true,
        rated_players: 1,
        pvb_pay: 0.5,
    }
}

/// Counters made with one kit.
fn counters_of(kit: usize, events: &[(usize, u16)]) -> PlayerStatistics {
    let mut statistics = PlayerStatistics::default();
    for &(event, count) in events {
        statistics.events[event][kit] = count;
    }
    statistics
}

/// A player who was in the match to its end.
fn player(place: u8, team: u8, name: &str, statistics: PlayerStatistics) -> PlayerResult {
    PlayerResult {
        place,
        session_id: 100 + u32::from(place),
        account_id: 10 + u32::from(place),
        name: name.to_owned(),
        team,
        driven: false,
        entered: true,
        present_at_end: true,
        left: false,
        time_in_match_ms: 600_000,
        score: snapshot::score(&statistics, &SCORE_WEIGHTS),
        kills: statistics.total(event::KILL) as u16,
        deaths: statistics.total(event::DEATH) as u16,
        longest_killstreak: statistics.longest_killstreak,
        kit_changes: 0,
        statistics: Some(statistics),
        pay_statistics: None,
        supplies: None,
    }
}

/// A battery match team 0 won 3 to 1, vouched for by two clients.
fn result(players: Vec<PlayerResult>) -> MatchResult {
    MatchResult {
        match_id: MATCH,
        map_id: 1,
        game_mode: mode::BATTERIES,
        match_level: 0,
        end: MatchEnd::ModeRule,
        winner: Some(0),
        team_points: [3, 1],
        duration_ms: 700_000,
        played_ms: Some(600_000),
        trusted_snapshot_ms: Some(690_000),
        trusted_by: 2,
        players,
        vs_bots: false,
        private: false,
        blackout: false,
    }
}

/// Alice (team 0): three kills, an assist and a death. Bob (team 1): a
/// kill and three deaths.
fn alice_and_bob() -> Vec<PlayerResult> {
    vec![
        player(
            0,
            0,
            "Alice",
            counters_of(
                0,
                &[(event::KILL, 3), (event::ASSIST, 1), (event::DEATH, 1)],
            ),
        ),
        player(
            1,
            1,
            "Bob",
            counters_of(0, &[(event::KILL, 1), (event::DEATH, 3)]),
        ),
    ]
}

/// Experience of an account that has played: more than a new account is
/// given (`tutorial::experience`), short of a level.
const PLAYED: u32 = 251;

/// The accounts of the players, none of them a new player.
fn accounts(players: &[PlayerResult]) -> GameAccounts {
    let store = GameAccounts::new();
    for player in players {
        let account = store.get_or_create(player.account_id, &player.name);
        let mut account = account.locked();
        only_the_pistol(&mut account);
        account.exp_total = PLAYED;
        // The day's three doubled victories are behind it (`bonus`): what
        // the tests below count is a plain match's pay.
        account.extras.bonus = spent_day();
    }
    store
}

/// A daily bonus whose three victories of the day of [`NOW`] are used up.
fn spent_day() -> crate::v026g::extras::DailyBonus {
    crate::v026g::extras::DailyBonus {
        day: 1,
        last_day: crate::v026g::bonus::day_of(NOW),
        wins: [3, 0, 0, 0, 0, 0, 0],
        reward_dict: PREMIUM_MONEY,
        reward_count: 50,
        ..Default::default()
    }
}

/// Of what a new account owns, the TT-33 in weapon slot 1 and its clip
/// only: what the tests below count the wear and the supplies of.
fn only_the_pistol(account: &mut Account) {
    let kept: Vec<u32> = [7, 8]
        .iter()
        .filter_map(|&slot| account.kits[0].slots[slot].map(|held| held.item_id))
        .collect();
    assert_eq!(kept.len(), 2);
    account
        .inventory
        .retain(|item| kept.contains(&item.item_id));
    account.sync_kits();
}

/// Every quest the Scavengers offer a new account rests: none is given.
fn no_new_quests(account: &mut Account) {
    account.quest_cooldowns = Quests::shared()
        .band(1, |_| 0)
        .map(|quest| (quest.id, NOW + 1_000_000))
        .collect();
}

fn account(store: &GameAccounts, account_id: u32) -> Account {
    store.get(account_id).unwrap().locked().clone()
}

/// The silver and the gold a reward's drop put in the balances.
fn dropped_money(reward: &Reward) -> (u32, u32) {
    match reward.drop {
        Some(drop) if drop.dict_id == GENERIC_MONEY => (drop.count, 0),
        Some(drop) if drop.dict_id == PREMIUM_MONEY => (0, drop.count),
        _ => (0, 0),
    }
}

/// The condition a weapon or armour found in a match arrives in: 40 %.
const FOUND_CONDITION: u32 = FULL_CONDITION / 100 * 40;

fn columns(full: &FullResult, line: usize) -> (f32, u32, i16, i16) {
    full.components[line].columns[0]
}

#[test]
fn the_winner_and_the_loser_are_paid_by_the_score() {
    let players = alice_and_bob();
    let store = accounts(&players);
    let before = account(&store, 10);
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();

    // Alice: 3 kills at 25 / 3, an assist at 10 / 1, and the fixed part of
    // a match won (82.73 / 122.69): no other score.
    let alice = &report.players[0];
    assert_eq!(alice.player_id, 1);
    assert_eq!(alice.reward.standing, Standing::Finished);
    assert!(alice.reward.won);
    assert_eq!(alice.reward.score, 35);
    assert_eq!(
        alice.full.components[0].name_key,
        "st_statistics_score_kills"
    );
    assert!(alice.full.components[0].show_amount);
    assert_eq!(columns(&alice.full, 0), (3.0, 75, 9, 0));
    assert_eq!(
        alice.full.components[1].name_key,
        "st_statistics_score_help"
    );
    assert_eq!(columns(&alice.full, 1), (1.0, 10, 1, 0));
    assert_eq!(
        alice.full.components[2].name_key,
        "st_statistics_score_game_events"
    );
    assert!(!alice.full.components[2].show_amount);
    assert_eq!(columns(&alice.full, 2), (2.0, 82, 122, 0));
    // The other kits were not used: nothing in their columns.
    for component in &alice.full.components {
        assert_eq!(component.columns[1], (0.0, 0, 0, 0));
        assert_eq!(component.columns[2], (0.0, 0, 0, 0));
    }
    assert_eq!((alice.reward.experience, alice.reward.money), (132, 167));
    // No quest faction: no reputation.
    assert_eq!(
        (alice.reward.reputation_faction, alice.reward.reputation),
        (0, 0)
    );
    assert_eq!(alice.full.faction, 0);
    assert!(!alice.reward.premium);
    // What premium would add, on the screen and not in the account.
    assert_eq!(alice.reward.premium_bonus, (83, 66, 0));
    assert_eq!(alice.full.premium.columns[0].money, 83);
    assert_eq!(alice.full.premium.columns[0].experience, 66);
    assert_eq!(alice.full.play_seconds, [600, 0, 0]);
    assert_eq!(
        (alice.reward.rating_before, alice.reward.rating_after),
        (100, 102)
    );
    assert_eq!((alice.full.elo_before, alice.full.elo_after), (100, 102));

    let after = account(&store, 10);
    let (silver, gold) = dropped_money(&alice.reward);
    assert_eq!(after.exp_total, before.exp_total + 132);
    assert_eq!(after.generic_money, before.generic_money + 167 + silver);
    assert_eq!(after.premium_money, before.premium_money + gold);
    assert_eq!((after.rating, after.rating_before), (102, 100));

    // Bob: a kill, and half the fixed part for a match lost and played to
    // its end (41.37 / 61.35).
    let bob = &report.players[1];
    assert!(!bob.reward.won);
    assert_eq!(columns(&bob.full, 0), (1.0, 25, 3, 0));
    assert_eq!(columns(&bob.full, 1), (0.0, 0, 0, 0));
    assert_eq!(columns(&bob.full, 2), (1.0, 41, 61, 0));
    assert_eq!((bob.reward.experience, bob.reward.money), (64, 66));
    assert_eq!(bob.reward.rating_after, 98);
    assert_eq!(account(&store, 11).rating, 98);

    // The screen.
    let stats = &report.stats;
    assert_eq!(stats.match_id, MATCH);
    assert_eq!(stats.winner_team, 0);
    assert_eq!(stats.team_scores, [3, 1]);
    assert_eq!(stats.team_params, [1, 1]);
    assert_eq!(stats.game_mode, mode::BATTERIES);
    assert_eq!(stats.best_player.name, "Alice");
    assert_eq!(stats.best_player_score, 35);
    // Nobody fired, helped or found anything the counters show.
    assert_eq!(stats.best_shooter, TitleHolder::default());
    assert_eq!(stats.best_explorer, TitleHolder::default());
    assert_eq!(stats.rows.len(), 2);
    let row = &stats.rows[1];
    assert_eq!(
        (
            row.name.as_str(),
            row.player_id,
            row.score,
            row.kills,
            row.deaths,
            row.team
        ),
        ("Bob", 2, 10, 1, 3, 1)
    );
}

#[test]
fn a_draw_pays_no_win_and_moves_no_rating() {
    let players = alice_and_bob();
    let store = accounts(&players);
    let mut result = result(players);
    result.winner = Some(DRAW);
    result.team_points = [2, 2];
    let report = Results::new()
        .settle(&result, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert_eq!(report.stats.winner_team, DRAW);
    for player in &report.players {
        assert!(!player.reward.won);
        assert_eq!(player.reward.rating_after, 100);
        // The fixed part of a match not won.
        assert_eq!(columns(&player.full, 2), (1.0, 41, 61, 0));
    }
    assert_eq!(report.players[0].reward.experience, 9 + 1 + 61);
}

#[test]
fn the_lowest_who_scored_thirty_get_eighty_percent_of_the_median() {
    // Silver of the rows: 12 kills won 300 + 82, 10 won 250 + 82, 9 won
    // 225 + 82, 3 lost 75 + 41, 1 lost 25 + 41. The median is 307: the
    // floor is 245.
    let kills = |place: u8, team: u8, kills: u16| {
        player(
            place,
            team,
            &format!("P{place}"),
            counters_of(0, &[(event::KILL, kills)]),
        )
    };
    let players = vec![
        kills(0, 0, 12),
        kills(1, 0, 10),
        kills(2, 0, 9),
        kills(3, 1, 3),
        kills(4, 1, 1),
    ];
    let store = accounts(&players);
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let money = |place: usize| report.players[place].reward.money;
    assert_eq!((money(0), money(1), money(2)), (382, 332, 307));
    // Score 30: the rest of the floor in its activity cell, 41 + 129.
    let low = &report.players[3];
    assert_eq!(low.reward.score, 30);
    assert_eq!(columns(&low.full, 0), (3.0, 75, 9, 0));
    assert_eq!(columns(&low.full, 2), (1.0, 170, 61, 0));
    assert_eq!(low.reward.money, 245);
    assert_eq!(low.full.premium.columns[0].money, 122);
    // Score 10: under the floor's score, paid as it was.
    assert_eq!(report.players[4].reward.score, 10);
    assert_eq!(money(4), 66);
}

#[test]
fn the_match_level_multiplies_the_rewards() {
    let players = alice_and_bob();
    let store = accounts(&players);
    let mut result = result(players);
    // Level 2: money 2, experience and reputation 1.05.
    result.match_level = 2;
    let report = Results::new()
        .settle(&result, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    // Each cell is multiplied and then rounded down: 75 silver becomes
    // 150, 9 experience 9.45; 82.73 silver 165.46, 122.69 experience
    // 128.8.
    assert_eq!(columns(&report.players[0].full, 0), (3.0, 150, 9, 0));
    assert_eq!(columns(&report.players[0].full, 1), (1.0, 20, 1, 0));
    assert_eq!(columns(&report.players[0].full, 2), (2.0, 165, 128, 0));
}

/// A player of team 0 with premium, a quest faction and the counters of a
/// results screen: `kills`, `assists` and an other score of `brought`
/// hundredths of a battery × 10.
fn screen_match(
    level: u16,
    kills: u16,
    assists: u16,
    brought: u16,
) -> (GameAccounts, MatchResult) {
    let players = vec![
        player(
            0,
            0,
            "Alice",
            counters_of(
                0,
                &[
                    (event::KILL, kills),
                    (event::ASSIST, assists),
                    (event::DEATH, 9),
                    (event::BRING_VICTORY_ITEM, brought),
                ],
            ),
        ),
        player(1, 1, "Bob", PlayerStatistics::default()),
    ];
    let store = accounts(&players);
    {
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        alice.premium_until = NOW + 3600;
        alice.quest_faction = 1;
        no_new_quests(&mut alice);
    }
    let mut result = result(players);
    result.match_level = level;
    (store, result)
}

/// The owner's second screen (LIKELY 0.27): match level 15, a win, score
/// 289 with 19 kills and 8 helps, premium, no daily bonus. All three rows,
/// the premium block and the totals come out.
#[test]
fn the_second_screen_comes_out() {
    // The other score, 59: five and nine tenths batteries brought (matched
    // as two artefacts and five and a half batteries while an artefact
    // weighed 2).
    let (store, result) = screen_match(15, 19, 8, 590);
    assert_eq!(result.players[0].score, 289);
    let report = Results::new()
        .settle(&result, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    assert_eq!(alice.reward.score, 289);
    assert_eq!(columns(&alice.full, 0), (19.0, 4830, 96, 32));
    assert_eq!(columns(&alice.full, 1), (8.0, 813, 13, 13));
    let (_, money, experience, reputation) = columns(&alice.full, 2);
    assert_eq!((money, experience, reputation), (5064, 270, 11));
    assert_eq!(
        alice.full.premium.columns[0],
        BonusColumn {
            money: 5353,
            reputation: 28,
            experience: 189,
            ..BonusColumn::default()
        }
    );
    assert_eq!(alice.full.daily, lobby::BonusBlock::default());
    assert_eq!(
        (alice.full.quest_money, alice.full.quest_reputation),
        (0, 0)
    );
    // The screen's money total, 9784, is these 16060 less the 6276 its
    // autobuy and repair spent: the client takes them off itself.
    assert_eq!(
        (
            alice.reward.money,
            alice.reward.experience,
            alice.reward.reputation
        ),
        (10_707 + 5_353, 568, 84)
    );
    assert_eq!(16_060 - 6_276, 9_784);
}

/// The owner's first screen (0.26): match level 1, a win, score 223 with
/// 17 kills and 4 helps, premium, one of the day's doubled victories. The
/// help and activity rows come out; the kills row cannot (seven of its
/// kills were of the full kind and ten of a lesser one, cause unknown:
/// `pay::LESSER_KILL`), and with it the totals' experience and silver. The
/// totals' shape does: the rows, the rows' experience and reputation once
/// more, and half of the rows.
#[test]
fn the_first_screen_comes_out_but_for_its_kills() {
    // The other score, 33: three and three tenths batteries brought
    // (matched as four artefacts and two and a half batteries while an
    // artefact weighed 2).
    let (store, result) = screen_match(1, 17, 4, 330);
    assert_eq!(result.players[0].score, 223);
    // Her first victory of the day.
    store.get(10).unwrap().locked().extras.bonus = Default::default();
    let report = Results::new()
        .settle(&result, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    assert_eq!(columns(&alice.full, 1), (4.0, 58, 4, 4));
    let (_, money, experience, reputation) = columns(&alice.full, 2);
    assert_eq!((money, experience, reputation), (463, 143, 5));
    // 17 kills of the full kind: 624.75, 51, 17 (the screen: 551, 41, 17).
    assert_eq!(columns(&alice.full, 0), (17.0, 624, 51, 17));
    let base = (624 + 58 + 463, 51 + 4 + 143, 17 + 4 + 5);
    // The reputation is the screen's: 26, +26 daily, +13 premium, 65.
    assert_eq!(base.2, 26);
    assert!(alice.reward.daily_bonus);
    assert_eq!(
        alice.full.daily.columns[0],
        BonusColumn {
            money: 0,
            experience: 198,
            reputation: 26,
            bonus_money: 0,
            bonus_experience: 100,
            bonus_reputation: 100,
        }
    );
    // Premium is half of the rows, not of the daily bonus.
    assert_eq!(
        alice.full.premium.columns[0],
        BonusColumn {
            money: 572,
            experience: 99,
            reputation: 13,
            ..BonusColumn::default()
        }
    );
    assert_eq!(
        (
            alice.reward.money,
            alice.reward.experience,
            alice.reward.reputation
        ),
        (base.0 + 572, base.1 + 198 + 99, 65)
    );
}

#[test]
fn the_money_rate_scales_the_rows_silver_and_premiums_half_of_it() {
    let players = alice_and_bob();
    let store = accounts(&players);
    store.get(10).unwrap().locked().quest_faction = 1;
    let terms = Terms {
        money_rate: pay::MoneyRate::Flat(80),
        experience_rate: 2,
        ..terms()
    };
    let report = Results::new()
        .settle(&result(players), &store, &terms, &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    // 80 % of 75, 10 and 82.73 silver; twice 9, 1 and 122.69 experience;
    // the reputation (3, 1 and 2.46) has its own rate.
    assert_eq!(columns(&alice.full, 0), (3.0, 60, 18, 3));
    assert_eq!(columns(&alice.full, 1), (1.0, 8, 2, 1));
    assert_eq!(columns(&alice.full, 2), (2.0, 66, 245, 2));
    assert_eq!(alice.reward.premium_bonus, (67, 132, 3));
    assert_eq!(alice.reward.money, 134);
    // srp's own terms: the owner's silver by match level, twice the
    // experience and 120 % of the reputation, no wear and no play time
    // bonus.
    let own = Terms::at(NOW);
    assert_eq!(
        (own.money_rate, own.experience_rate, own.reputation_rate),
        (pay::MoneyRate::ByLevel, 2, 120)
    );
    assert_eq!((own.wear, own.playtime_bonus), (false, false));
    assert_eq!(own.score_weights, SCORE_WEIGHTS);
}

/// The owner's rates (2026-10-14): the rows' silver at the percent of the
/// match's level, their experience twice, the rows' and the quests'
/// reputation at 120 %.
#[test]
fn the_owners_rates_raise_silver_by_match_level_experience_and_reputation() {
    let settle = |level: u16, terms: &Terms| {
        let players = alice_and_bob();
        let store = accounts(&players);
        {
            // The Scavengers' first quest (win with the base not robbed):
            // 50 silver, 50 reputation.
            let alice = store.get(10).unwrap();
            let mut alice = alice.locked();
            quests::choose_faction(&mut alice, 1).unwrap();
            alice.quests.push(QuestInstance {
                instance_id: 50,
                quest_id: 10101,
                games: 0,
                expires_at: NOW + 10_000,
            });
            alice.next_quest_instance = 51;
            no_new_quests(&mut alice);
        }
        let mut result = result(players);
        result.match_level = level;
        let report = Results::new()
            .settle(&result, &store, terms, &mut Rng::new(5))
            .unwrap();
        report.players[0].clone()
    };
    let own = Terms {
        money_rate: pay::MoneyRate::ByLevel,
        experience_rate: 2,
        reputation_rate: 120,
        ..terms()
    };
    // Level 0 (every multiplier 1): 150 % of 75, 10 and 82.73 silver;
    // twice 9, 1 and 122.69 experience; 120 % of 3, 1 and 2.46 reputation.
    let alice = settle(0, &own);
    assert_eq!(columns(&alice.full, 0), (3.0, 112, 18, 3));
    assert_eq!(columns(&alice.full, 1), (1.0, 15, 2, 1));
    assert_eq!(columns(&alice.full, 2), (2.0, 124, 245, 2));
    // Level 8 (`money_k` 5.59, the others 1.35): 180 % of the silver, and
    // twice 12.15, 1.35 and 165.63 experience.
    let plain = settle(8, &terms());
    let alice = settle(8, &own);
    assert_eq!(
        (0..3)
            .map(|line| columns(&alice.full, line).2)
            .collect::<Vec<_>>(),
        [24, 2, 331]
    );
    for line in 0..3 {
        let (_, money, _, _) = columns(&alice.full, line);
        let (_, plain_money, _, _) = columns(&plain.full, line);
        assert!(
            (f64::from(money) - 1.8 * f64::from(plain_money)).abs() <= 2.0,
            "{line}: {money} against {plain_money}"
        );
    }
    assert_eq!(
        columns(&alice.full, 0).1,
        (75.0 * 5.59 * 1.8_f64).floor() as u32
    );
    // 3 × 1.35 × 1.2 = 4.86, 1.62 and 3.98.
    assert_eq!(
        (0..3)
            .map(|line| columns(&alice.full, line).3)
            .collect::<Vec<_>>(),
        [4, 1, 3]
    );
    // From level 12 on: 220 %.
    for level in [12, 15, 20] {
        let k = pay::multiplier(Dictionary::shared().match_level(level).unwrap().money_k);
        assert_eq!(
            columns(&settle(level as u16, &own).full, 0).1,
            pay::cell(75.0, k * 2.2) as u32
        );
    }
    // The quest's silver is the match level's alone (50 × 5.59); its
    // reputation is 120 % of the level's (50 × 1.35 = 67.5, × 1.2 = 81).
    assert_eq!(
        (plain.reward.quest_money, alice.reward.quest_money),
        (279, 279)
    );
    assert_eq!(plain.reward.quest_reputation, [(1, 67)]);
    assert_eq!(alice.reward.quest_reputation, [(1, 81)]);
    assert_eq!(alice.full.quest_reputation, 81);
}

#[test]
fn one_who_came_late_moves_the_balance_rating_by_its_share_of_the_match() {
    // Three against two for the whole match, team 0 the winner.
    let whole = |players: Vec<PlayerResult>| {
        let store = accounts(&players);
        Results::new()
            .settle(&result(players), &store, &terms(), &mut Rng::new(1))
            .unwrap();
        store
    };
    let from_the_start = whole(three_and_two());
    // The same, but Erin (team 0) came for the last tenth of it: two
    // against two for most of it, so the win says more of Alice, and
    // little of Erin.
    let mut players = three_and_two();
    players[4].time_in_match_ms /= 10;
    let late = whole(players);
    let alice = |store: &GameAccounts| account(store, 10).balance.rating;
    let erin = |store: &GameAccounts| account(store, 14).balance.rating;
    assert!(alice(&late) > alice(&from_the_start));
    assert!(erin(&late) > 0 && erin(&late) * 5 < alice(&late));
    assert_eq!(erin(&from_the_start), alice(&from_the_start));
    // The match is one more of hers all the same.
    assert_eq!(account(&late, 14).balance.matches, 1);
}

#[test]
fn one_who_left_gets_no_finish_bonus_and_no_drop_and_loses_rating() {
    // Carol left the winning team early; Dave's game never got in.
    let mut players = alice_and_bob();
    let mut carol = player(2, 0, "Carol", counters_of(0, &[(event::KILL, 1)]));
    carol.present_at_end = false;
    carol.left = true;
    carol.time_in_match_ms = 90_000;
    let mut dave = player(3, 1, "Dave", PlayerStatistics::default());
    dave.entered = false;
    dave.present_at_end = false;
    dave.time_in_match_ms = 0;
    dave.statistics = None;
    players.extend([carol, dave]);
    let store = accounts(&players);
    let dave_before = account(&store, 13);
    // New players: a drop for each who stayed.
    for id in 10..14 {
        store.get(id).unwrap().locked().exp_total = 0;
    }
    let dave_before = Account {
        exp_total: 0,
        ..dave_before
    };
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(9))
        .unwrap();

    let carol = &report.players[2].reward;
    assert_eq!(carol.standing, Standing::Left);
    // Its team won, and it left early: its kill, and nothing of the
    // activity row's fixed part.
    assert!(carol.won);
    assert_eq!(columns(&report.players[2].full, 0), (1.0, 25, 3, 0));
    assert_eq!(columns(&report.players[2].full, 2), (0.0, 0, 0, 0));
    assert_eq!((carol.experience, carol.money), (3, 25));
    // A loss in the rating, though its team won.
    assert!(carol.rating_after < carol.rating_before);
    assert_eq!(carol.drop, None);
    assert_eq!(carol.drop_chance, 0.0);
    assert_eq!(report.players[2].full.dropped, None);
    assert_eq!(report.players[2].full.play_seconds, [90, 0, 0]);
    // Its pistol was in a match.
    assert_eq!(carol.worn.len(), 1);

    let dave = &report.players[3];
    assert_eq!(dave.reward.standing, Standing::Absent);
    assert_eq!(
        dave.reward,
        Reward {
            standing: Standing::Absent,
            rating_before: 100,
            rating_after: 100,
            ..Reward::default()
        }
    );
    assert_eq!(account(&store, 13), dave_before);
    // It is still a row of the screen, and gets one.
    assert_eq!(report.stats.rows.len(), 4);
    assert!(report.screen_for(13).unwrap().is_ok());

    // Those who stayed, new players both, found something.
    for index in [0, 1] {
        let reward = &report.players[index].reward;
        let drop = reward.drop.unwrap();
        assert_eq!(reward.drop_chance, 1.0);
        let params = item_params(drop.dict_id).unwrap();
        assert!((1..=2).contains(&params.item_level));
        // An item in the inventory, with the three modifications of the
        // rarity the new player's table always rolls; found in a match, so
        // at 40 % and with one defect.
        assert_eq!((drop.count, drop.rarity, drop.rolled_rarity), (1, 3, 3));
        let owner = account(&store, 10 + index as u32);
        let item = owner.item(drop.item_id).unwrap();
        assert_eq!(crate::v026g::items_rules::rank(item), 3);
        assert_eq!(crate::v026g::items_rules::defects(item).len(), 1);
        assert_eq!(
            (item.dict_id, item.condition_or_stack),
            (drop.dict_id, FOUND_CONDITION)
        );
        let shown = report.players[index].full.dropped.as_ref().unwrap();
        assert_eq!((shown.item_id, shown.dict_id), (drop.item_id, drop.dict_id));
        assert_eq!(
            report.players[index].full.drop_chances,
            [1.0, 0.0, 0.0, 0.0]
        );
    }
    // Teams of two and two: the leaver counts in its team's average.
    assert_eq!(report.stats.team_params, [2, 2]);
}

/// Bob left the match by his own word (place 1: a kill, three deaths) and
/// came into it again as a new place (place 2), fresh: only that place is
/// his, settled once; the one he left counts for nothing (the owner's of
/// 2026-10-19).
#[test]
fn one_who_left_and_came_again_is_settled_once_by_its_latest_place() {
    let mut players = alice_and_bob();
    players[1].present_at_end = false;
    players[1].left = true;
    players[1].time_in_match_ms = 200_000;
    let mut again = player(2, 1, "Bob", PlayerStatistics::default());
    again.account_id = players[1].account_id;
    again.session_id = players[1].session_id;
    again.time_in_match_ms = 300_000;
    players.push(again);
    let store = accounts(&players);
    let before = account(&store, 11);
    let result = result(players);
    let latest = result.latest_places();
    assert_eq!(
        latest
            .players
            .iter()
            .map(|player| (player.place, player.account_id))
            .collect::<Vec<_>>(),
        [(0, 10), (2, 11)]
    );
    let report = Results::new()
        .settle(&result, &store, &terms(), &mut Rng::new(1))
        .unwrap();

    // One report and one row of the screen each.
    assert_eq!(report.players.len(), 2);
    assert_eq!(report.stats.rows.len(), 2);
    assert_eq!(report.index_of(11), Some(1));
    let bob = &report.players[1];
    assert_eq!(bob.player_id, 3);
    // His latest place stayed to the end and lost: no kill of the place he
    // left, the fixed part of a match lost to its end, and the rating of a
    // loss once.
    assert_eq!(bob.reward.standing, Standing::Finished);
    assert_eq!(columns(&bob.full, 0), (0.0, 0, 0, 0));
    assert_eq!(columns(&bob.full, 2), (1.0, 41, 61, 0));
    assert_eq!((bob.reward.experience, bob.reward.money), (61, 41));
    assert_eq!(
        (bob.reward.rating_before, bob.reward.rating_after),
        (100, 98)
    );
    let after = account(&store, 11);
    assert_eq!(after.rating, 98);
    assert_eq!(after.exp_total, before.exp_total + bob.reward.experience);
    assert_eq!(after.balance.matches, 1);
    assert_eq!(report.stats.team_params, [1, 1]);
}

#[test]
fn a_solo_match_pays_in_full_and_moves_no_rating() {
    let players = vec![player(
        0,
        0,
        "Alice",
        // Two artefacts, the first into an empty slot (event 5 and 16),
        // the second with every slot full (event 5 alone).
        counters_of(
            0,
            &[(event::ARTEFACT, 2), (event::WINNING_TEAM_VI_BONUS, 1)],
        ),
    )];
    let store = accounts(&players);
    let mut result = result(players);
    result.trusted_by = 1;
    let report = Results::new()
        .settle(&result, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    assert!(alice.reward.won);
    // The two artefacts are 15 + 5 points of other score (140.8 silver,
    // 12.3 experience) on the fixed part of a match won.
    assert_eq!(alice.reward.score, 20);
    assert_eq!(columns(&alice.full, 2), (5.0, 223, 135, 0));
    assert_eq!(alice.reward.experience, 135);
    assert_eq!(alice.reward.rating_after, 100);
    assert_eq!(account(&store, 10).rating, 100);
    assert_eq!(report.stats.rows.len(), 1);
    assert_eq!(report.stats.best_explorer.name, "Alice");
    assert_eq!(report.stats.explorer_artefacts, 2);
}

#[test]
fn experience_reaches_levels_and_they_pay() {
    let players = alice_and_bob();
    let store = accounts(&players);
    store.get(10).unwrap().locked().exp_total = 400;
    let before = account(&store, 10);
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let reward = &report.players[0].reward;
    // 400 + 132 is past the 500 of level 1, which pays 100 gold.
    assert_eq!(reward.levels, [1]);
    assert_eq!((reward.level_money, reward.level_gold), (0, 100));
    let after = account(&store, 10);
    assert_eq!(after.level(), 1);
    assert_eq!(after.free_skill_points(), 1);
    let (silver, gold) = dropped_money(reward);
    assert_eq!(after.premium_money, before.premium_money + 100 + gold);
    assert_eq!(after.generic_money, before.generic_money + 167 + silver);
    // Bob reached none.
    assert!(report.players[1].reward.levels.is_empty());
    // Level 2 pays silver.
    let mut account = Account::new("Eve");
    assert_eq!(pay_levels(&mut account, 1250), (vec![1, 2], 500, 100));
    // The last level is the hundredth, and pays 250 gold.
    let mut account = Account::new("Eve");
    account.exp_total = 1_287_499;
    assert_eq!(account.level(), 99);
    assert_eq!(pay_levels(&mut account, 1), (vec![100], 0, 250));
    assert_eq!(pay_levels(&mut account, 1_000_000), (vec![], 0, 0));
    assert_eq!(account.premium_money, 250);
}

#[test]
fn a_quest_the_match_meets_is_paid_and_reputation_goes_to_the_quest_faction() {
    let quests = Quests::shared();
    let players = alice_and_bob();
    let store = accounts(&players);
    // Alice takes the Scavengers' quests and holds their first: win with
    // the base not robbed.
    let quest = quests.quest(10101).unwrap();
    {
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        quests::choose_faction(&mut alice, 1).unwrap();
        alice.quests.push(QuestInstance {
            instance_id: 50,
            quest_id: quest.id,
            games: 0,
            expires_at: NOW + 10_000,
        });
        alice.next_quest_instance = 51;
        // Six points short of the Scavengers' level 1.
        alice.add_reputation(1, 94);
    }
    let before = account(&store, 10);
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(5))
        .unwrap();
    let alice = &report.players[0];
    // The rows' reputation (3 kills, an assist, 2.46 of the activity),
    // with the faction chosen.
    assert_eq!(alice.reward.reputation_faction, 1);
    assert_eq!(alice.reward.reputation, 6);
    assert_eq!(columns(&alice.full, 0), (3.0, 75, 9, 3));
    assert_eq!(columns(&alice.full, 1), (1.0, 10, 1, 1));
    assert_eq!(columns(&alice.full, 2), (2.0, 82, 122, 2));
    assert_eq!(alice.full.faction, 1);
    // The quest.
    assert!(alice.reward.quest_money >= quest.money);
    assert_eq!(alice.full.quest_money, alice.reward.quest_money);
    let points: u32 = alice
        .reward
        .quest_reputation
        .iter()
        .filter(|&&(faction, _)| faction == 1)
        .map(|&(_, points)| points)
        .sum();
    assert!(points >= quest.reputation[0].1);
    assert_eq!(u32::from(alice.full.quest_reputation), points);
    assert!(
        alice
            .notices
            .contains(&(QUEST_MESSAGE, "#q![50][10101]".to_owned()))
    );
    // 100 points with the Scavengers is their level 1.
    assert_eq!(alice.reward.reputation_levels, [1]);
    assert!(
        alice
            .notices
            .contains(&(REPUTATION_LEVEL_MESSAGE, "#ru[1:1]".to_owned()))
    );
    let after = account(&store, 10);
    assert_eq!(after.reputation(1), 94 + 6 + points);
    let (silver, _) = dropped_money(&alice.reward);
    assert_eq!(
        after.generic_money,
        before.generic_money + 167 + alice.reward.quest_money + silver
    );
    assert!(after.quests.iter().all(|held| held.quest_id != 10101));
    // Bob lost: his quests, had he any, are not met; he has no faction.
    assert_eq!(report.players[1].reward.quest_money, 0);
    assert_eq!(report.players[1].reward.reputation, 0);
}

/// A quest's award is multiplied by the level of the match it was
/// completed in, and its reputation doubled in one of the day's doubled
/// victories.
#[test]
fn a_quests_award_follows_the_match() {
    let quests = Quests::shared();
    let quest = quests.quest(10101).unwrap();
    let settle = |level: u16, fresh_day: bool| {
        let players = alice_and_bob();
        let store = accounts(&players);
        {
            let alice = store.get(10).unwrap();
            let mut alice = alice.locked();
            quests::choose_faction(&mut alice, 1).unwrap();
            alice.quests.push(QuestInstance {
                instance_id: 50,
                quest_id: quest.id,
                games: 0,
                expires_at: NOW + 10_000,
            });
            alice.next_quest_instance = 51;
            no_new_quests(&mut alice);
            if fresh_day {
                alice.extras.bonus = Default::default();
            }
        }
        let mut result = result(players);
        result.match_level = level;
        let report = Results::new()
            .settle(&result, &store, &terms(), &mut Rng::new(5))
            .unwrap();
        let reward = report.players[0].reward.clone();
        assert_eq!(reward.daily_bonus, fresh_day);
        let points: u32 = reward
            .quest_reputation
            .iter()
            .filter(|&&(faction, _)| faction == 1)
            .map(|&(_, points)| points)
            .sum();
        assert_eq!(u32::from(report.players[0].full.quest_reputation), points);
        assert_eq!(report.players[0].full.quest_money, reward.quest_money);
        (reward.quest_money, points)
    };
    let (money, points) = (quest.money, quest.reputation[0].1);
    assert_eq!(settle(0, false), (money, points));
    let plain = (money, points);
    // Level 2: `money_k` 2, `reputation_k` 1.05, rounded down.
    let level_2 = settle(2, false);
    assert_eq!(level_2, (2 * money, points * 105 / 100));
    // One of the day's first three victories: the reputation twice, the
    // money as it was.
    assert_eq!(settle(2, true), (level_2.0, 2 * level_2.1));
    assert_eq!(settle(0, true), (plain.0, 2 * plain.1));
}

#[test]
fn the_quest_numbers_come_from_the_counters() {
    let terms = terms();
    let mut alice = player(
        0,
        0,
        "Alice",
        counters_of(
            1,
            &[
                (event::KILL, 7),
                (event::DEATH, 2),
                (event::HEADSHOT_KILL, 3),
                (event::MELEE_KILL, 1),
                (GRENADE_KILL, 2),
                (ENTRAP_ENEMY, 4),
                (event::BULLET_FIRE, 200),
                (event::BULLET_HIT, 50),
                (event::ARTEFACT, 5),
                // Five batteries put into the container; two of them found
                // and carried by Alice alone.
                (event::PUT_VICTORY_ITEM_IN_CONTAINER, 5),
                (event::SOLO_DELIVERY, 2),
                // Two and a half batteries' worth of credit for stolen
                // ones; one of them put into the container by Alice.
                (event::STEAL_VICTORY_ITEM, 250),
                (event::STOLEN_AND_DELIVERED, 1),
            ],
        ),
    );
    alice.longest_killstreak = 4;
    let bob = player(1, 1, "Bob", PlayerStatistics::default());
    let result = result(vec![alice, bob]);
    let shared = Shared::of(&result, &terms);
    assert_eq!(
        shared.quest_stats(&result.players[0]),
        QuestStats {
            won: true,
            kills: 7,
            deaths: 2,
            headshots: 3,
            max_killstreak: 4,
            grenade_kills: 2,
            melee_kills: 1,
            trapped: 4,
            shots: 200,
            hits: 50,
            artefacts: 5,
            delivered: 2,
            stolen: 1,
            team_batteries: 3,
            base_robbed: false,
            battery_mode: true,
        }
    );
    // Alice stole from Bob's base.
    let bob = shared.quest_stats(&result.players[1]);
    assert!(bob.base_robbed);
    assert!(!bob.won);
    assert_eq!(bob.team_batteries, 1);
    // Tickets are no batteries.
    let mut deathmatch = result.clone();
    deathmatch.game_mode = mode::TEAM_DEATHMATCH;
    let shared = Shared::of(&deathmatch, &terms);
    let alice = shared.quest_stats(&deathmatch.players[0]);
    assert_eq!(alice.team_batteries, 0);
    assert!(!alice.battery_mode);
    // Nor are Research's points, and no base is robbed there.
    let mut research = result.clone();
    research.game_mode = mode::RESEARCH;
    let shared = Shared::of(&research, &terms);
    let alice = shared.quest_stats(&research.players[0]);
    assert_eq!(alice.team_batteries, 0);
    assert!(!alice.battery_mode);
    assert!(!shared.quest_stats(&research.players[1]).base_robbed);
}

/// A Research match (mode 5) team 0 won 100 to 64: Alice captured units
/// worth fifteen whole captures (more than a map has batteries),
/// neutralised one and killed two invaders.
fn research_match(game_mode: u8, weights: &[f32; EVENTS]) -> (GameAccounts, MatchResult) {
    let mut alice = player(
        0,
        0,
        "Alice",
        counters_of(
            0,
            &[
                (event::RESEARCH_CAPTURE, 1500),
                (event::RESEARCH_NEUTRALISE, 100),
                (event::KILL_INVADER, 2),
            ],
        ),
    );
    alice.score = snapshot::score(alice.statistics.as_ref().unwrap(), weights);
    let players = vec![alice, player(1, 1, "Bob", PlayerStatistics::default())];
    let store = accounts(&players);
    let mut result = result(players);
    result.game_mode = game_mode;
    result.team_points = [100, 64];
    (store, result)
}

#[test]
fn a_research_match_pays_its_captures_by_research_weights_uncapped() {
    let research_weights = static_info::score_weights(mode::RESEARCH);
    // The score the match server sent and counted: 15 × 30 + 30 + 2 × 10.
    let (store, result) = research_match(mode::RESEARCH, &research_weights);
    assert_eq!(result.players[0].score, 500);
    let report = Results::new()
        .settle(&result, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    assert!(alice.reward.won);
    assert_eq!(alice.reward.score, 500);
    assert_eq!(report.stats.game_mode, mode::RESEARCH);
    assert_eq!(
        (report.stats.winner_team, report.stats.team_scores),
        (0, [100, 64])
    );
    // The activity row: the 18 events and the fixed part of a win.
    let activity = columns(&alice.full, 2);
    assert_eq!(activity.0, 20.0);
    // As a match whose terms weigh so pays it: all fifteen captures, at
    // Research's weights and not the batteries'.
    let (store, plain) = research_match(mode::BATTERIES, &research_weights);
    let weighed = Terms {
        score_weights: research_weights,
        ..terms()
    };
    let as_weighed = Results::new()
        .settle(&plain, &store, &weighed, &mut Rng::new(1))
        .unwrap();
    assert_eq!(columns(&as_weighed.players[0].full, 2), activity);
    let (store, batteries) = research_match(mode::BATTERIES, &SCORE_WEIGHTS);
    let at_battery_weights = Results::new()
        .settle(&batteries, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert!(columns(&at_battery_weights.players[0].full, 2).1 < activity.1);
    // Research's points are no batteries: nothing to the drop chance.
    assert_eq!(alice.full.drop_chances[1], 0.0);
    assert!(as_weighed.players[0].full.drop_chances[1] > 0.0);
}

#[test]
fn drops_follow_the_seed_and_the_play_time() {
    let dictionary = Dictionary::shared();
    let players = alice_and_bob();
    let settle = |seed: u64| {
        let store = accounts(&players);
        let before = account(&store, 10);
        let report = Results::new()
            .settle(
                &result(players.clone()),
                &store,
                &terms(),
                &mut Rng::new(seed),
            )
            .unwrap();
        (report, before, account(&store, 10))
    };
    let (mut some, mut none, mut gear) = (0, 0, 0);
    for seed in 1..=60 {
        let (report, before, after) = settle(seed);
        // The same seed, the same match.
        assert_eq!(*settle(seed).0, *report);
        let alice = &report.players[0];
        // The first game of a play period: twice the table's 40 %; and
        // 2.5 % for each of her team's three batteries. No premium, no
        // penalty.
        assert!((alice.full.drop_chances[0] - 0.8).abs() < 1e-6);
        assert!((alice.full.drop_chances[1] - 0.075).abs() < 1e-6);
        assert_eq!(alice.full.drop_chances[2..], [0.0, 0.0]);
        assert!((alice.reward.drop_chance - 0.875).abs() < 1e-6);
        // Bob's team has one.
        let bob = &report.players[1].full.drop_chances;
        assert!((bob[1] - 0.025).abs() < 1e-6, "{bob:?}");
        let Some(drop) = alice.reward.drop else {
            none += 1;
            assert_eq!(alice.full.dropped, None);
            assert_eq!(after.inventory.len(), before.inventory.len());
            continue;
        };
        some += 1;
        let shown = alice.full.dropped.as_ref().unwrap();
        assert_eq!(shown.dict_id, drop.dict_id);
        match drop.dict_id {
            GENERIC_MONEY => {
                assert_eq!(drop.item_id, 0);
                assert_eq!(shown.condition_or_stack, drop.count);
                assert_eq!(
                    after.generic_money,
                    before.generic_money + alice.reward.money + drop.count
                );
                assert_eq!(after.inventory.len(), before.inventory.len());
            }
            PREMIUM_MONEY => {
                assert_eq!(after.premium_money, before.premium_money + drop.count);
                assert_eq!(after.inventory.len(), before.inventory.len());
            }
            dict_id if dictionary.is_stack(dict_id) => {
                let item = after.item(drop.item_id).unwrap();
                assert_eq!(item.condition_or_stack, drop.count);
                assert_eq!(alice.full.drop_rarity, drop.rolled_rarity);
            }
            _ => {
                let item = after.item(drop.item_id).unwrap();
                // A weapon or armour has the modifications of its rarity,
                // and the screen is told how many. Found in a match, it is
                // at 40 % and has one defect; a cosmetic is whole.
                let rank = crate::v026g::items_rules::rank(item);
                let defects = crate::v026g::items_rules::defects(item).len();
                if is_gear(drop.dict_id) {
                    gear += 1;
                    assert_eq!(item.condition_or_stack, FOUND_CONDITION);
                    assert_eq!(defects, 1);
                } else {
                    assert_eq!(item.condition_or_stack, FULL_CONDITION);
                    assert_eq!((rank, defects), (0, 0));
                }
                assert_eq!(usize::from(alice.full.drop_rarity), rank);
                assert!(rank <= usize::from(drop.rolled_rarity));
                let shown = alice.full.dropped.as_ref().unwrap();
                assert_eq!(shown.modifications.len(), rank + defects);
                assert_eq!(shown.condition_or_stack, item.condition_or_stack);
                assert_eq!(after.inventory.len(), before.inventory.len() + 1);
            }
        }
    }
    assert!(some > 30 && none > 0, "{some} drops, {none} none");
    assert!(gear > 5, "{gear} weapons and armour");

    // The play time bonus: 100, 75, 50, 25 % more for the first four games
    // of a period, none after; an hour without a match starts another.
    let store = accounts(&players);
    let results = Results::new();
    let mut rng = Rng::new(3);
    let mut chances = Vec::new();
    let mut at = NOW;
    for game in 0..7 {
        if game == 5 {
            at += 3600;
        }
        at += 900;
        let mut result = result(players.clone());
        result.match_id = MATCH + game;
        let terms = Terms { now: at, ..terms() };
        let report = results.settle(&result, &store, &terms, &mut rng).unwrap();
        chances.push((report.players[0].full.drop_chances[0] * 100.0).round() as u32);
    }
    assert_eq!(chances, [80, 70, 60, 50, 40, 80, 70]);

    // With the bonus off (srp's own terms) every game has the table's
    // plain chance.
    let store = accounts(&players);
    let results = Results::new();
    let mut rng = Rng::new(3);
    let mut chances = Vec::new();
    for game in 0..3 {
        let mut result = result(players.clone());
        result.match_id = MATCH + game;
        let terms = Terms {
            now: NOW + 900 * u64::from(game),
            playtime_bonus: false,
            ..terms()
        };
        let report = results.settle(&result, &store, &terms, &mut rng).unwrap();
        chances.push((report.players[0].full.drop_chances[0] * 100.0).round() as u32);
    }
    assert_eq!(chances, [40, 40, 40]);
}

#[test]
fn the_drop_chance_has_its_three_parts() {
    // Team 0 has `batteries` at the end; Alice has premium or not; the
    // fifth game of a play period, so the table's plain 40 %.
    let parts = |batteries: u16, premium: bool, mode: u8| {
        let players = alice_and_bob();
        let store = accounts(&players);
        if premium {
            store.get(10).unwrap().locked().premium_until = NOW + 3600;
        }
        let results = Results::new();
        let mut rng = Rng::new(11);
        let mut last = None;
        for game in 0..5 {
            let mut result = result(players.clone());
            result.match_id = MATCH + game;
            result.game_mode = mode;
            result.team_points = [batteries, 0];
            last = results.settle(&result, &store, &terms(), &mut rng);
        }
        let report = last.unwrap();
        let alice = &report.players[0];
        let sum: f32 = alice.full.drop_chances[..3].iter().sum();
        assert!((alice.reward.drop_chance - sum).abs() < 1e-6);
        alice
            .full
            .drop_chances
            .map(|part| (part * 1000.0).round() as u32)
    };
    assert_eq!(parts(0, false, mode::BATTERIES), [400, 0, 0, 0]);
    assert_eq!(parts(5, false, mode::BATTERIES), [400, 125, 0, 0]);
    assert_eq!(parts(5, true, mode::BATTERIES), [400, 125, 250, 0]);
    assert_eq!(parts(0, true, mode::BATTERIES), [400, 0, 250, 0]);
    // All ten home and premium: 40 + 25 + 25.
    assert_eq!(parts(10, true, mode::BATTERIES), [400, 250, 250, 0]);
    // Tickets are no batteries.
    assert_eq!(parts(40, true, mode::TEAM_DEATHMATCH), [400, 0, 250, 0]);
    // The parts are sent as the client reads them: four f32 at the end of
    // the screen.
    let players = alice_and_bob();
    let store = accounts(&players);
    store.get(10).unwrap().locked().premium_until = NOW + 3600;
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    // The first game: 80 %, 7.5 % for three batteries, and what is left
    // of 100 for premium.
    let message = report.screen_for(10).unwrap().unwrap();
    let body = message.body();
    let mut tail = Vec::new();
    for part in [0.8_f32, 0.075, 0.125, 0.0] {
        tail.extend(part.to_le_bytes());
    }
    let sent: Vec<f32> = body[body.len() - 16..]
        .chunks(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
        .collect();
    for (sent, part) in sent.iter().zip([0.8_f32, 0.075, 0.125, 0.0]) {
        assert!((sent - part).abs() < 1e-6, "{sent} is not {part}");
    }
    assert_eq!(body[body.len() - 16..body.len() - 12], tail[..4]);
    assert_eq!(body[body.len() - 4..], [0, 0, 0, 0]);
}

/// A weapon found in a match: 40 % and one defect the client knows for
/// it, byte for byte as the screen (`97`) and the inventory (`89`) carry
/// it.
#[test]
fn a_weapon_found_in_a_match_is_worn_and_has_a_defect() {
    let dictionary = Dictionary::shared();
    let table = modifications::Modifications::shared();
    let toz = dictionary.item_named("toz_34").unwrap().dict_id;
    assert!(is_gear(toz));
    let rolled = drops::Drop {
        dict_id: toz,
        count: 1,
        rarity: 0,
    };
    let candidates: Vec<u32> = table
        .defect_candidates(toz, &[], &[])
        .iter()
        .map(|row| row.id)
        .collect();
    assert!(!candidates.is_empty());
    let mut defects_seen = Vec::new();
    for seed in 1..=40 {
        let mut account = Account::new("Alice");
        let mut rng = Rng::new(seed);
        let (dropped, shown) = give_drop(&mut account, &rolled, true, &mut rng);
        let item = account.item(dropped.item_id).unwrap().clone();
        assert_eq!(item.condition_or_stack, 400_000);
        // No modification (rarity 0), one defect, of those the item may
        // have: an id the client's defect table has.
        assert_eq!(item.modifications.len(), 1);
        let (defect, state) = item.modifications[0];
        assert!(state);
        assert!(table.is_known(defect, true));
        assert!(candidates.contains(&defect));
        if !defects_seen.contains(&defect) {
            defects_seen.push(defect);
        }
        assert_eq!(dropped.rarity, 0);

        // The descriptor of the screen: id, dict id, condition, one time
        // (no repair, no change of modifications), one `{id, state 1}`,
        // no upgrades.
        let mut expected = Vec::new();
        expected.extend(dropped.item_id.to_le_bytes());
        expected.extend(toz.to_le_bytes());
        expected.extend([0x80, 0x1A, 0x06, 0x00]);
        expected.extend([0, 0, 0, 0]);
        expected.push(1);
        expected.extend(defect.to_le_bytes());
        expected.push(1);
        expected.push(0);
        let mut full = FullResult::empty(1);
        full.dropped = Some(shown);
        let stats = lobby::MatchStats {
            match_id: MATCH,
            unknown: 0,
            winner_team: 0,
            team_scores: [0, 0],
            team_params: [0, 0],
            game_mode: mode::BATTERIES,
            best_player: TitleHolder::default(),
            best_player_score: 0,
            best_shooter: TitleHolder::default(),
            shooter_accuracy_percent: 0,
            shooter_shots: 0,
            best_support: TitleHolder::default(),
            support_value: 0,
            best_explorer: TitleHolder::default(),
            explorer_artefacts: 0,
            explorer_items: 0,
            rows: Vec::new(),
        };
        let screen = lobby::match_stats(&stats, &full).unwrap();
        let body = screen.body();
        // After the descriptor: the rarity and the four chances.
        let end = body.len() - 17;
        assert_eq!(body[end - expected.len()..end], expected[..], "seed {seed}");
        // The client reads the screen and the inventory with the item.
        let mut client = Client::signed_in();
        let Decoded::MatchStats { own, .. } = client.receive(&screen).unwrap() else {
            panic!("not the screen");
        };
        let read = own.dropped.unwrap();
        assert_eq!(read.condition_or_stack, 400_000);
        assert_eq!(read.modifications.len(), 1);
        // The inventory's descriptor has the repair time too.
        let inventory = wire::after_change(&account, NOW).unwrap();
        let mut expected = Vec::new();
        expected.extend(dropped.item_id.to_le_bytes());
        expected.extend(toz.to_le_bytes());
        expected.extend([0x80, 0x1A, 0x06, 0x00]);
        expected.extend([0; 8]);
        expected.push(1);
        expected.extend(defect.to_le_bytes());
        expected.extend([1, 0]);
        let body = inventory[0].body();
        assert!(
            body.windows(expected.len())
                .any(|window| window == expected),
            "seed {seed}"
        );
        client.receive_all(&inventory).unwrap();
        assert!(
            client
                .inventory()
                .iter()
                .any(|held| held.item_id == dropped.item_id && held.condition_or_stack == 400_000)
        );

        // From a drop card the same weapon is whole.
        let (card, _) = give_drop(&mut account, &rolled, false, &mut rng);
        let item = account.item(card.item_id).unwrap();
        assert_eq!(item.condition_or_stack, FULL_CONDITION);
        assert!(item.modifications.is_empty());
    }
    // The defect is rolled among those the weapon may have.
    assert!(
        defects_seen.len() > 1 || candidates.len() == 1,
        "{defects_seen:?} of {candidates:?}"
    );
    // A cosmetic has no condition to lose.
    let camouflage = dictionary
        .items
        .iter()
        .find(|item| !item.is_stack && !is_gear(item.dict_id) && item.dict_id > 215)
        .map(|item| item.dict_id);
    if let Some(dict_id) = camouflage {
        let mut account = Account::new("Alice");
        let rolled = drops::Drop {
            dict_id,
            count: 1,
            rarity: 0,
        };
        let (dropped, _) = give_drop(&mut account, &rolled, true, &mut Rng::new(1));
        let item = account.item(dropped.item_id).unwrap();
        assert_eq!(item.condition_or_stack, FULL_CONDITION);
        assert!(item.modifications.is_empty());
    }
}

#[test]
fn a_stack_dropped_goes_onto_the_stack_owned() {
    let mut account = Account::new("Alice");
    let card = Dictionary::shared()
        .item_named("drop_card")
        .unwrap()
        .dict_id;
    let rolled = drops::Drop {
        dict_id: card,
        count: 2,
        rarity: 0,
    };
    let mut rng = Rng::new(1);
    let (first, shown) = give_drop(&mut account, &rolled, true, &mut rng);
    assert_eq!(shown, ItemDescr::plain(first.item_id, card, 2));
    let (second, shown) = give_drop(&mut account, &rolled, true, &mut rng);
    assert_eq!(second.item_id, first.item_id);
    // The message carries what dropped, the inventory the sum.
    assert_eq!(shown.condition_or_stack, 2);
    assert_eq!(account.item(first.item_id).unwrap().condition_or_stack, 4);
}

/// With wear off (srp's own terms) a match leaves every item as it was:
/// nothing worn, no defect, nothing for auto-repair to pay; the ammo is
/// still used.
#[test]
fn with_wear_off_a_match_leaves_the_items_whole() {
    let mut players = alice_and_bob();
    players[0].statistics.as_mut().unwrap().events[event::BULLET_FIRE][0] = 100;
    let store = accounts(&players);
    let pistol = {
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        alice.extras.auto_repair = true;
        alice.kits[0].slots[7].unwrap().item_id
    };
    let terms = Terms {
        wear: false,
        ..terms()
    };
    let report = Results::new()
        .settle(&result(players), &store, &terms, &mut Rng::new(1))
        .unwrap();
    for player in &report.players {
        assert!(player.reward.worn.is_empty());
        assert!(player.reward.defects.is_empty());
        assert!(player.reward.spent.repair.is_empty());
    }
    let alice = account(&store, 10);
    assert_eq!(
        alice.item(pistol).unwrap().condition_or_stack,
        FULL_CONDITION
    );
    assert_eq!(report.players[0].reward.supplies_used, [(50, 50)]);
}

#[test]
fn the_kit_played_wears() {
    let mut players = alice_and_bob();
    // Alice fired 100 bullets with her first kit.
    players[0].statistics.as_mut().unwrap().events[event::BULLET_FIRE][0] = 100;
    let store = accounts(&players);
    // A helmet in Alice's second kit, which she did not use.
    let helmet = Dictionary::shared()
        .item_named("ren_helmet_1")
        .unwrap()
        .dict_id;
    let (pistol, ammo, spare) = {
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        let spare = alice.add_item(helmet, FULL_CONDITION);
        alice.kits[1].slots[1] = Some(crate::v026g::KitSlot {
            item_id: spare,
            count: 1,
        });
        // Autobuy off (on for a new account): the ammo used stays used.
        alice.autobuy = false;
        (
            alice.kits[0].slots[7].unwrap().item_id,
            alice.kits[0].slots[8].unwrap().item_id,
            spare,
        )
    };
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    // The TT-33: 1 % a match and 0.05 % a shot.
    let worn = &report.players[0].reward.worn;
    assert_eq!(worn.len(), 1);
    let (item_id, before, after) = worn[0];
    assert_eq!((item_id, before), (pistol, FULL_CONDITION));
    assert!((939_990..=940_010).contains(&after), "{after}");
    let alice = account(&store, 10);
    assert_eq!(alice.item(pistol).unwrap().condition_or_stack, after);
    // Stacks do not wear. The bullets the counters saw were the pistol's
    // one kind of ammo (`autobuy`): no more than the 50 rounds she owned,
    // so the stack is used up and its slot empty.
    assert_eq!(report.players[0].reward.supplies_used, [(50, 50)]);
    assert!(alice.item(ammo).is_none());
    assert_eq!(alice.kits[0].slots[8], None);
    assert_eq!(
        alice.item(spare).unwrap().condition_or_stack,
        FULL_CONDITION
    );
    // Bob fired nothing: the match's 1 %.
    let (_, _, after) = report.players[1].reward.worn[0];
    assert!((989_990..=990_010).contains(&after), "{after}");
    // The kits other players see carry the wear.
    assert_eq!(
        report.players[0].body.items[0].condition_or_stack,
        worn[0].2
    );

    // Counters of the second kit wear it too.
    let mut players = alice_and_bob();
    players[0].statistics.as_mut().unwrap().events[event::DEATH][1] = 1;
    let store = accounts(&players);
    let spare = {
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        let spare = alice.add_item(helmet, FULL_CONDITION);
        alice.kits[1].slots[1] = Some(crate::v026g::KitSlot {
            item_id: spare,
            count: 1,
        });
        spare
    };
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert_eq!(report.players[0].reward.worn.len(), 2);
    assert!(account(&store, 10).item(spare).unwrap().condition_or_stack < FULL_CONDITION);
}

#[test]
fn premium_adds_half_and_can_be_claimed_from_the_screen() {
    let players = alice_and_bob();
    let store = accounts(&players);
    {
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        alice.premium_until = NOW + 3600;
        quests::choose_faction(&mut alice, 2).unwrap();
    }
    quests::choose_faction(&mut store.get(11).unwrap().locked(), 2).unwrap();
    let results = Results::new();
    let report = results
        .settle(&result(players), &store, &terms(), &mut Rng::new(2))
        .unwrap();
    // Alice has premium: half again of her rows' 167 silver, 132
    // experience and 6 reputation.
    let alice = &report.players[0];
    assert!(alice.reward.premium);
    assert_eq!(alice.reward.premium_bonus, (83, 66, 3));
    assert_eq!(
        (
            alice.reward.money,
            alice.reward.experience,
            alice.reward.reputation
        ),
        (167 + 83, 132 + 66, 6 + 3)
    );
    // The lines hold the plain values, the premium block the half: the
    // client adds them up.
    assert_eq!(columns(&alice.full, 0), (3.0, 75, 9, 3));
    assert_eq!(
        alice.full.premium.columns[0],
        BonusColumn {
            money: 83,
            reputation: 3,
            experience: 66,
            ..BonusColumn::default()
        }
    );
    assert_eq!(account(&store, 10).exp_total, PLAYED + 198);
    assert_eq!(alice.full.daily, lobby::BonusBlock::default());
    assert_eq!(alice.full.premium_items, lobby::BonusBlock::default());

    // Bob has none: the block says what it would give.
    let bob = &report.players[1];
    assert_eq!(bob.reward.premium_bonus, (33, 32, 1));
    assert_eq!((bob.reward.money, bob.reward.experience), (66, 64));
    let bob_account = store.get(11).unwrap();
    let mut bob_account = bob_account.locked();
    let before = bob_account.clone();
    // Not without premium; then once.
    assert_eq!(
        results.claim_premium(MATCH, 11, &mut bob_account, NOW),
        None
    );
    bob_account.premium_until = NOW + 60;
    let (bonus, notices) = results
        .claim_premium(MATCH, 11, &mut bob_account, NOW)
        .unwrap();
    assert_eq!(bonus, (33, 32, 1));
    assert!(notices.is_empty());
    assert_eq!(bob_account.generic_money, before.generic_money + 33);
    assert_eq!(bob_account.exp_total, before.exp_total + 32);
    assert_eq!(bob_account.reputation(2), before.reputation(2) + 1);
    assert_eq!(
        results.claim_premium(MATCH, 11, &mut bob_account, NOW),
        None
    );
    // Alice was paid with it.
    let alice_account = store.get(10).unwrap();
    assert_eq!(
        results.claim_premium(MATCH, 10, &mut alice_account.locked(), NOW),
        None
    );
    assert_eq!(
        results.claim_premium(MATCH + 1, 11, &mut bob_account, NOW),
        None
    );
}

#[test]
fn without_a_trusted_snapshot_only_staying_is_paid() {
    let mut players = alice_and_bob();
    for player in &mut players {
        player.statistics = None;
        player.score = 0;
        player.kills = 0;
        player.deaths = 0;
    }
    let store = accounts(&players);
    let mut result = result(players);
    result.winner = None;
    result.team_points = [0, 0];
    result.trusted_snapshot_ms = None;
    result.trusted_by = 0;
    result.end = MatchEnd::TimeLimit;
    let report = Results::new()
        .settle(&result, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert_eq!(report.stats.winner_team, NO_WINNER);
    assert_eq!(report.stats.best_player, TitleHolder::default());
    for (index, player) in report.players.iter().enumerate() {
        assert_eq!(player.reward.standing, Standing::Finished);
        assert!(!player.reward.won);
        assert_eq!(player.reward.score, 0);
        assert_eq!(columns(&player.full, 0), (0.0, 0, 0, 0));
        assert_eq!(columns(&player.full, 2), (1.0, 41, 61, 0));
        assert_eq!((player.reward.experience, player.reward.money), (61, 41));
        assert_eq!(player.reward.rating_after, 100);
        // The rating's "before" is not touched by a match that moved none.
        assert_eq!(account(&store, 10 + index as u32).rating_before, 100);
        // No standing known: no part of the drop chance for batteries.
        assert_eq!(player.full.drop_chances[1], 0.0);
        // A drop is still rolled, and the kit was in a match.
        assert!(player.reward.drop_chance > 0.0);
        assert_eq!(player.reward.worn.len(), 1);
    }

    // Numbers no client vouched for are not used, even if they are there.
    let players = alice_and_bob();
    let store = accounts(&players);
    let mut result = super::tests::result(players);
    result.trusted_by = 0;
    let report = Results::new()
        .settle(&result, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert_eq!(report.stats.winner_team, NO_WINNER);
    assert_eq!(report.stats.team_scores, [0, 0]);
    assert_eq!(report.players[0].reward.experience, 61);
    assert_eq!(report.stats.rows[0].kills, 0);
}

#[test]
fn a_match_nobody_reached_or_never_played_pays_nothing() {
    let players = alice_and_bob();
    let store = accounts(&players);
    let before = account(&store, 10);
    let results = Results::new();

    let mut no_show = result(players.clone());
    no_show.end = MatchEnd::NoShow;
    assert!(
        results
            .settle(&no_show, &store, &terms(), &mut Rng::new(1))
            .is_none()
    );
    assert!(!results.has_unseen(10));

    // The match waited for players and never began.
    let mut waited = result(players);
    waited.end = MatchEnd::TimeLimit;
    waited.played_ms = None;
    let report = results
        .settle(&waited, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    for player in &report.players {
        assert_eq!(player.reward.standing, Standing::NotPlayed);
        assert_eq!(player.reward.experience, 0);
        assert_eq!(player.reward.drop, None);
        assert!(player.reward.worn.is_empty());
    }
    assert_eq!(account(&store, 10), before);
    assert_eq!(report.stats.winner_team, NO_WINNER);
    // A screen all the same.
    assert!(results.has_unseen(10));
    assert!(report.screen_for(10).unwrap().is_ok());
}

#[test]
fn an_account_the_store_lacks_is_a_row_and_nothing_else() {
    let players = alice_and_bob();
    let store = GameAccounts::new();
    store.get_or_create(10, "Alice").locked().exp_total = PLAYED;
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert_eq!(report.stats.rows.len(), 2);
    assert_eq!(report.players[1].reward, Reward::default());
    // One team with a rating: none moves.
    assert_eq!(report.players[0].reward.rating_after, 100);
    assert!(store.get(11).is_none());
    assert!(report.screen_for(11).unwrap().is_ok());
    let mut client = Client::signed_in();
    client
        .receive(&report.player_details(2).unwrap().unwrap())
        .unwrap();
}

#[test]
fn reports_are_shown_once_and_kept_for_asking() {
    let players = alice_and_bob();
    let store = accounts(&players);
    quests::choose_faction(&mut store.get(10).unwrap().locked(), 1).unwrap();
    let results = Results::new();
    assert!(results.take_unseen(10).is_none());
    let report = results
        .settle(&result(players.clone()), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    // Alice was given quests and reached a reputation level.
    assert!(!report.notices_for(10).is_empty());
    assert!(report.notices_for(99).is_empty());
    assert!(results.has_unseen(10));
    // A second match before she is shown the first.
    let mut second = result(players);
    second.match_id = MATCH + 1;
    let later = Terms {
        now: NOW + 900,
        ..terms()
    };
    let second = results
        .settle(&second, &store, &later, &mut Rng::new(2))
        .unwrap();
    let unseen = results.take_unseen(10).unwrap();
    // The last screen, and the notices of both.
    assert_eq!(unseen.report.match_id, MATCH + 1);
    let mut notices = report.notices_for(10).to_vec();
    notices.extend(second.notices_for(10).iter().cloned());
    assert_eq!(unseen.notices, notices);
    assert!(results.take_unseen(10).is_none());
    assert!(!results.has_unseen(10));
    // Bob has his own.
    assert_eq!(results.take_unseen(11).unwrap().report.match_id, MATCH + 1);

    // Asked for again.
    let found = results.find(MATCH).unwrap();
    assert_eq!(found.match_id, MATCH);
    assert_eq!(found.index_of(11), Some(1));
    assert!(found.screen_for(10).unwrap().is_ok());
    assert!(found.screen_for(99).is_none());
    assert!(found.player_details(2).unwrap().is_ok());
    assert!(found.player_details(9).is_none());
    assert!(results.find(0x999).is_none());

    // Only the last reports are kept.
    for index in 0..KEPT as u32 {
        let mut next = result(alice_and_bob());
        next.match_id = 0x1000 + index;
        results.settle(&next, &store, &later, &mut Rng::new(3));
    }
    assert!(results.find(MATCH).is_none());
    assert!(results.find(0x1000).is_some());
    assert_eq!(
        results.take_unseen(10).unwrap().report.match_id,
        0x1000 + KEPT as u32 - 1
    );
}

#[test]
fn a_drop_card_is_used_for_one_roll() {
    let dictionary = Dictionary::shared();
    let card = dictionary.item_named("drop_card").unwrap().dict_id;
    let mut account = Account::new("Alice");
    let cards = account.add_item(card, 2);
    let pistol = account.kits[0].slots[7].unwrap().item_id;
    let mut client = Client::signed_in();
    client
        .receive_all(&wire::after_change(&account, NOW).unwrap())
        .unwrap();
    let mut rng = Rng::new(4);
    // Not a card, not an item.
    assert_eq!(
        use_drop_card(&mut account, pistol, 5, &mut rng).err(),
        Some(DropCardRefusal::NoCard)
    );
    assert!(use_drop_card(&mut account, 999, 5, &mut rng).is_err());

    for left in [1, 0] {
        let before = account.clone();
        let used = use_drop_card(&mut account, cards, 5, &mut rng)
            .unwrap()
            .unwrap();
        assert_eq!(used.cards_left, left);
        // A cosmetic, armour or a weapon, into the inventory.
        let item = dictionary.item(used.dropped.dict_id).unwrap();
        assert!(
            matches!(item.category, 1..=8 | 13..=17 | 25 | 26),
            "{}",
            item.name
        );
        assert!(account.item(used.dropped.item_id).is_some());
        assert_eq!(
            account
                .item(cards)
                .map_or(0, |item| item.condition_or_stack),
            left
        );
        assert_eq!(account.generic_money, before.generic_money);
        // The client takes the message, and then holds what the account
        // does (but for a card stack at 0, which the next inventory drops).
        let Decoded::Drop {
            item: shown,
            card_item_id,
            card_count,
            ..
        } = client.receive(&used.message).unwrap()
        else {
            panic!("not a drop");
        };
        assert_eq!((card_item_id, card_count), (cards, left));
        assert_eq!(shown.dict_id, used.dropped.dict_id);
        assert_eq!(
            client
                .inventory()
                .iter()
                .filter(|item| item.condition_or_stack > 0)
                .count(),
            account.inventory.len()
        );
        client
            .receive_all(&wire::after_change(&account, NOW).unwrap())
            .unwrap();
        assert_eq!(client.inventory().len(), account.inventory.len());
    }
    // No card left.
    assert!(use_drop_card(&mut account, cards, 5, &mut rng).is_err());
}

/// Alice and Bob, who finished the match; Carol, who left it at once (the
/// 1-player PIN-room screen of 2026-10-20); Dave, whose game never got in.
fn finishers_and_not() -> Vec<PlayerResult> {
    let mut players = alice_and_bob();
    let mut carol = player(2, 0, "Carol", PlayerStatistics::default());
    carol.present_at_end = false;
    carol.left = true;
    carol.time_in_match_ms = 2_000;
    let mut dave = player(3, 1, "Dave", PlayerStatistics::default());
    dave.entered = false;
    dave.present_at_end = false;
    dave.statistics = None;
    players.extend([carol, dave]);
    players
}

/// Settles `MATCH` (level 7) with [`finishers_and_not`] and gives every
/// player and Erin (account 14, not in the match) three drop cards: the
/// cards' item ids by account.
fn match_with_cards(store: &GameAccounts, results: &Results) -> HashMap<u32, u32> {
    let card = Dictionary::shared().item_named(DROP_CARD).unwrap().dict_id;
    store.get_or_create(14, "Erin");
    let mut cards = HashMap::new();
    for id in 10..=14 {
        cards.insert(id, store.get(id).unwrap().locked().add_item(card, 3));
        store.mark_dirty(id);
    }
    let mut result = result(finishers_and_not());
    result.match_level = 7;
    results
        .settle(&result, store, &at(NOW), &mut Rng::new(3))
        .unwrap();
    store.flush();
    cards
}

/// [`Results::use_drop_card`] on the account as the store has it: `(the
/// match's level, the cards left)`. A refusal changes nothing of it.
fn use_card_of(
    results: &Results,
    store: &GameAccounts,
    match_id: u32,
    id: u32,
    item_id: u32,
    rng: &mut Rng,
) -> Result<(u16, u32), DropCardRefusal> {
    let account = store.get(id).unwrap();
    let mut account = account.locked();
    let before = account.clone();
    let used = results.use_drop_card(match_id, id, &mut account, item_id, rng);
    if used.is_err() {
        assert_eq!(*account, before);
    }
    used.map(|(level, used)| (level, used.unwrap().cards_left))
}

/// One drop card for each player that finished a match, once (the owner's
/// rule of 2026-10-20, `drop_cards.md`): the mark is kept with the account,
/// so a restart does not open the screen again.
#[test]
fn a_drop_card_is_taken_once_for_each_finisher_of_a_match_across_a_restart() {
    let players = finishers_and_not();
    let repository = MemoryGameRepository::default();
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    let cards = match_with_cards(&store, &results);
    let mut rng = Rng::new(3);
    let marks = |store: &GameAccounts, id: u32| store.get(id).unwrap().locked().drop_cards.clone();

    // Erin was not in the match; Carol left it at once; Dave never got
    // in; no match is kept under that id; no card has that id.
    assert_eq!(
        use_card_of(&results, &store, MATCH, 14, cards[&14], &mut rng),
        Err(DropCardRefusal::NotAPlayer)
    );
    assert_eq!(
        use_card_of(&results, &store, MATCH, 12, cards[&12], &mut rng),
        Err(DropCardRefusal::NotFinished(Standing::Left))
    );
    assert_eq!(
        use_card_of(&results, &store, MATCH, 13, cards[&13], &mut rng),
        Err(DropCardRefusal::NotFinished(Standing::Absent))
    );
    assert_eq!(
        use_card_of(&results, &store, MATCH + 9, 10, cards[&10], &mut rng),
        Err(DropCardRefusal::NoMatch)
    );
    assert_eq!(
        use_card_of(&results, &store, MATCH, 10, 999, &mut rng),
        Err(DropCardRefusal::NoCard)
    );
    assert!(marks(&store, 10).is_empty());
    // Alice's first card is a roll at the match's level; a second is not.
    assert_eq!(
        use_card_of(&results, &store, MATCH, 10, cards[&10], &mut rng),
        Ok((7, 2))
    );
    assert_eq!(marks(&store, 10), [MATCH]);
    assert_eq!(
        use_card_of(&results, &store, MATCH, 10, cards[&10], &mut rng),
        Err(DropCardRefusal::Used)
    );
    store.mark_dirty(10);
    store.flush();
    drop(results);
    drop(store);

    // srp starts again on the same database: her card stays used, Bob's
    // is still his to use, once.
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    assert_eq!(marks(&store, 10), [MATCH]);
    assert_eq!(
        use_card_of(&results, &store, MATCH, 10, cards[&10], &mut rng),
        Err(DropCardRefusal::Used)
    );
    assert_eq!(
        use_card_of(&results, &store, MATCH, 11, cards[&11], &mut rng),
        Ok((7, 2))
    );
    assert_eq!(
        use_card_of(&results, &store, MATCH, 11, cards[&11], &mut rng),
        Err(DropCardRefusal::Used)
    );
}

/// A repository whose results rows stop being written once `stop` is set:
/// srp ending after an account's write and before the results' rows.
struct ResultsStop {
    inner: MemoryGameRepository,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl GameRepository for ResultsStop {
    fn load_all(&mut self) -> Result<Vec<(u32, Account)>, crate::persist::PersistError> {
        self.inner.load_all()
    }
    fn save(
        &mut self,
        account_id: u32,
        account: &Account,
    ) -> Result<(), crate::persist::PersistError> {
        self.inner.save(account_id, account)
    }
    fn describe(&self) -> String {
        "results stop".to_owned()
    }
    fn load_results(&mut self) -> Result<Vec<(String, String)>, crate::persist::PersistError> {
        self.inner.load_results()
    }
    fn save_result(&mut self, key: &str, body: &str) -> Result<(), crate::persist::PersistError> {
        if self.stop.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(crate::persist::PersistError::GameData("stopped".to_owned()));
        }
        self.inner.save_result(key, body)
    }
    fn delete_result(&mut self, key: &str) -> Result<(), crate::persist::PersistError> {
        self.inner.delete_result(key)
    }
}

/// srp ends after the account that used a card is written and before the
/// results' rows are: the card taken, the item given and the mark are one
/// write, so after the restart the card is still refused for the match.
#[test]
fn a_crash_after_the_accounts_write_takes_no_second_card() {
    let players = finishers_and_not();
    let repository = MemoryGameRepository::default();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let store = GameAccounts::with_repository(Box::new(ResultsStop {
        inner: repository.clone(),
        stop: Arc::clone(&stop),
    }))
    .unwrap();
    for player in &players {
        let account = store.get_or_create(player.account_id, &player.name);
        let mut account = account.locked();
        only_the_pistol(&mut account);
        account.exp_total = PLAYED;
        account.extras.bonus = spent_day();
    }
    let results = Results::attached(&store);
    // The report and the state are written.
    let cards = match_with_cards(&store, &results);
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut rng = Rng::new(5);
    assert_eq!(
        use_card_of(&results, &store, MATCH, 10, cards[&10], &mut rng),
        Ok((7, 2))
    );
    let used = store.get(10).unwrap().locked().clone();
    store.mark_dirty(10);
    store.flush();
    // The account is written; the results' rows are not.
    assert_eq!(repository.get(10).unwrap(), used);
    drop(results);
    drop(store);

    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    assert!(results.find(MATCH).is_some());
    assert_eq!(*store.get(10).unwrap().locked(), used);
    assert_eq!(
        use_card_of(&results, &store, MATCH, 10, cards[&10], &mut rng),
        Err(DropCardRefusal::Used)
    );
}

/// A roll that gives nothing (a broken `drop_card_drop`) is its own
/// refusal: the card is kept and no mark is made, so the next card works.
#[test]
fn a_roll_that_gives_nothing_keeps_the_card_and_the_match() {
    let players = finishers_and_not();
    let store = accounts(&players);
    let results = Results::new();
    let cards = match_with_cards(&store, &results);
    let empty = DropTable {
        categories: Vec::new(),
        ..Drops::shared().drop_card.clone()
    };
    let mut rng = Rng::new(6);
    let alice = store.get(10).unwrap();
    let mut alice = alice.locked();
    let before = alice.clone();
    assert_eq!(
        use_card(&mut alice, cards[&10], 7, &empty, &mut rng).err(),
        Some(DropCardRefusal::NoRoll)
    );
    assert_eq!(
        results
            .use_drop_card_from(&empty, MATCH, 10, &mut alice, cards[&10], &mut rng)
            .err(),
        Some(DropCardRefusal::NoRoll)
    );
    assert_eq!(*alice, before);
    let (level, used) = results
        .use_drop_card(MATCH, 10, &mut alice, cards[&10], &mut rng)
        .unwrap();
    assert_eq!((level, used.unwrap().cards_left), (7, 2));
    assert_eq!(alice.drop_cards, [MATCH]);
}

/// The marks are of kept reports only: one whose report is gone goes with
/// the next card, so the list stays as short as the reports kept.
#[test]
fn the_marks_of_matches_no_longer_kept_go() {
    let players = finishers_and_not();
    let store = accounts(&players);
    let results = Results::new();
    let cards = match_with_cards(&store, &results);
    let mut rng = Rng::new(7);
    assert_eq!(
        use_card_of(&results, &store, MATCH, 10, cards[&10], &mut rng),
        Ok((7, 2))
    );
    for index in 0..KEPT as u32 {
        let mut next = result(alice_and_bob());
        next.match_id = 0x1000 + index;
        results.settle(&next, &store, &at(NOW), &mut Rng::new(3));
    }
    assert!(results.find(MATCH).is_none());
    let last = 0x1000 + KEPT as u32 - 1;
    assert_eq!(
        use_card_of(&results, &store, last, 10, cards[&10], &mut rng),
        Ok((0, 1))
    );
    assert_eq!(store.get(10).unwrap().locked().drop_cards, [last]);
}

/// Every player's pushes, as the lobby sends them, through the client's own
/// reading rules: a client that signed in before the match, then what the
/// match changed, the screen, the notices and a click on every row.
#[test]
fn the_client_takes_every_players_pushes() {
    // Four players: Alice with premium, a quest faction and counters in two
    // kits; Bob with a clan tag; Carol who left; Dave who never got in.
    let mut alice = player(
        0,
        0,
        "Alice",
        counters_of(
            0,
            &[
                (event::KILL, 5),
                (event::ASSIST, 2),
                (event::TEAMMATE_CURE, 1),
                (event::BRING_VICTORY_ITEM, 150),
                (event::BULLET_FIRE, 300),
                (event::BULLET_HIT, 90),
                (event::ARTEFACT, 1),
                (event::VICTORY_ITEM_FOUND, 2),
            ],
        ),
    );
    alice.statistics.as_mut().unwrap().events[event::KILL][1] = 2;
    alice.score = snapshot::score(alice.statistics.as_ref().unwrap(), &SCORE_WEIGHTS);
    let bob = player(1, 1, "Bob", counters_of(2, &[(event::KILL, 1)]));
    let mut carol = player(2, 0, "Carol", counters_of(0, &[(event::ASSIST, 1)]));
    carol.present_at_end = false;
    carol.left = true;
    let mut dave = player(3, 1, "Dave", PlayerStatistics::default());
    dave.entered = false;
    dave.present_at_end = false;
    dave.statistics = None;
    let players = vec![alice, bob, carol, dave];

    for seed in 1..=25 {
        let store = accounts(&players);
        {
            let alice = store.get(10).unwrap();
            let mut alice = alice.locked();
            alice.premium_until = NOW + 86_400;
            quests::choose_faction(&mut alice, 1).unwrap();
            quests::refresh(&mut alice, NOW, &mut Rng::new(seed));
            // Close to a level and to a reputation level.
            alice.exp_total = 450;
            alice.add_reputation(1, 90);
        }
        {
            let bob = store.get(11).unwrap();
            let mut bob = bob.locked();
            bob.clan_tag = "SRP".to_owned();
            // A new player: the new player's table.
            bob.exp_total = 0;
        }
        // Each client as it was before the match.
        let mut clients: Vec<Client> = (10..14)
            .map(|id| {
                let mut client = Client::new();
                let account = account(&store, id);
                client
                    .receive_all(&wire::sign_in_push(&account, &wire::SignInInfo::at(NOW)).unwrap())
                    .unwrap();
                client
            })
            .collect();

        let results = Results::new();
        let mut result = result(players.clone());
        result.match_level = (seed % 21) as u16;
        if seed % 5 == 0 {
            result.game_mode = mode::TEAM_DEATHMATCH;
            result.team_points = [40, 0];
        }
        let report = results
            .settle(&result, &store, &terms(), &mut Rng::new(seed))
            .unwrap();
        assert_eq!(report.stats.rows.len(), 4);
        assert_eq!(report.stats.rows[1].clan_tag, "SRP");
        assert_eq!(report.stats.best_shooter.name, "Alice");
        assert_eq!(report.stats.shooter_accuracy_percent, 30);
        assert_eq!(report.stats.best_support.name, "Alice");

        let mut message_id = 1;
        for (index, client) in clients.iter_mut().enumerate() {
            let account_id = 10 + index as u32;
            let account = account(&store, account_id);
            let unseen = results.take_unseen(account_id).unwrap();
            let push = unseen
                .report
                .lobby_push(account_id, &account, NOW, Some(index as u32 + 1))
                .unwrap()
                .unwrap();
            // The inventory first, the screen last.
            assert_eq!(push[0].id(), lobby::ServerId::Inventory);
            assert_eq!(push.last().unwrap().id(), lobby::ServerId::MatchStats);
            let read = client
                .receive_all(&push)
                .unwrap_or_else(|error| panic!("seed {seed}, player {index}: {error}"));
            assert_eq!(client.inventory().len(), account.inventory.len());
            let Some(Decoded::MatchStats { stats, own }) = read.last() else {
                panic!("not the screen");
            };
            assert_eq!(*stats, report.stats);
            assert_eq!(*own, report.players[index].full);
            // The client finds its own row by the full result's id.
            assert_eq!(stats.rows[index].player_id, own.player_id);
            assert_eq!(stats.rows[index].name, account.name);
            // What the screen adds up is what the account got.
            let reward = &report.players[index].reward;
            let mut shown = (0_u32, 0_u32, 0_u32);
            for component in &own.components {
                for &(_, money, experience, reputation) in &component.columns {
                    shown.0 += money;
                    shown.1 += experience as u32;
                    shown.2 += reputation as u32;
                }
            }
            if reward.premium {
                for column in &own.premium.columns {
                    shown.0 += column.money;
                    shown.1 += u32::from(column.experience);
                    shown.2 += u32::from(column.reputation);
                }
            }
            assert_eq!(
                shown,
                (reward.money, reward.experience, reward.reputation),
                "seed {seed}, player {index}"
            );

            // The notices, numbered as the lobby does.
            for (kind, text) in &unseen.notices {
                client
                    .receive(&lobby::important_message(
                        2, 0, "", "", *kind, text, message_id,
                    ))
                    .unwrap();
                message_id += 1;
            }
            // The answer to `3C 00` after the screen, and a click on
            // every row.
            client.receive(&wire::quests(&account, NOW)).unwrap();
            for row in &report.stats.rows {
                let details = report.player_details(row.player_id).unwrap().unwrap();
                let Decoded::PlayerDetails { player, .. } = client.receive(&details).unwrap()
                else {
                    panic!("not the player's details");
                };
                assert_eq!(player.name, row.name);
            }
            // The screen alone, as after a sign-in or `41 00`.
            client
                .receive(&report.screen_for(account_id).unwrap().unwrap())
                .unwrap();
        }
        // Alice: a level and a reputation level, on two kits' columns.
        let alice = &report.players[0];
        assert!(!alice.reward.levels.is_empty());
        assert!(!alice.reward.reputation_levels.is_empty());
        assert!(columns(&alice.full, 0).0 == 5.0);
        assert_eq!(alice.full.components[0].columns[1].0, 2.0);
        // A battery and a half brought, an artefact, two found, staying
        // and the win.
        assert_eq!(columns(&alice.full, 2).0, 1.5 + 1.0 + 2.0 + 2.0);
        // Bob played his third kit; the bonus is with the kit he has
        // active.
        let bob = &report.players[1];
        assert_eq!(bob.full.components[0].columns[2].0, 1.0);
        assert_eq!(columns(&bob.full, 2).0, 1.0);
        assert!(bob.reward.drop.is_some());
        if result.game_mode == mode::TEAM_DEATHMATCH {
            assert_eq!(report.stats.team_scores, [40, 0]);
        }
    }
}

// ---- the daily bonus, the level notice, the supplies and the two boxes ----

#[test]
fn the_days_first_three_victories_pay_double() {
    let players = alice_and_bob();
    // What the match pays with the day's bonus used up.
    let plain_store = accounts(&players);
    plain_store.get(10).unwrap().locked().quest_faction = 3;
    let plain = Results::new()
        .settle(
            &result(players.clone()),
            &plain_store,
            &terms(),
            &mut Rng::new(1),
        )
        .unwrap();
    let plain = &plain.players[0];
    assert_eq!((plain.reward.experience, plain.reward.reputation), (132, 6));
    assert!(!plain.reward.daily_bonus);
    assert_eq!(plain.full.daily, lobby::BonusBlock::default());

    // Neither has played today; Alice takes Renaissance quests.
    let store = accounts(&players);
    for id in [10, 11] {
        let account = store.get(id).unwrap();
        let mut account = account.locked();
        account.extras.bonus = Default::default();
        account.quest_faction = 3;
    }
    let results = Results::new();
    let report = results
        .settle(&result(players.clone()), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    assert!(alice.reward.daily_bonus);
    // The experience and the reputation once more; the money as it was.
    assert_eq!(alice.reward.experience, 2 * plain.reward.experience);
    assert_eq!(alice.reward.reputation, 2 * plain.reward.reputation);
    assert_eq!(alice.reward.money, plain.reward.money);
    let daily = alice.full.daily.columns[0];
    assert_eq!(u32::from(daily.experience), plain.reward.experience);
    assert_eq!(u32::from(daily.reputation), plain.reward.reputation);
    assert_eq!(
        (
            daily.money,
            daily.bonus_money,
            daily.bonus_experience,
            daily.bonus_reputation
        ),
        (0, 0, 100, 100)
    );
    // The kits she did not play have no column.
    assert_eq!(alice.full.daily.columns[1], lobby::BonusColumn::default());
    // What the screen adds up is what the account got.
    let mut shown = (0_u32, 0_u32);
    for component in &alice.full.components {
        for &(_, _, experience, reputation) in &component.columns {
            shown.0 += experience as u32;
            shown.1 += reputation as u32;
        }
    }
    for column in &alice.full.daily.columns {
        shown.0 += u32::from(column.experience);
        shown.1 += u32::from(column.reputation);
    }
    assert_eq!(shown, (alice.reward.experience, alice.reward.reputation));
    let after = account(&store, 10);
    assert_eq!(after.exp_total, PLAYED + alice.reward.experience);
    assert_eq!(crate::v026g::bonus::progress(&after), 1);
    assert_eq!(after.extras.bonus.day, 1);
    // The screen goes out in a form the client takes.
    let mut client = Client::signed_in();
    assert!(
        client
            .receive(&report.screen_for(10).unwrap().unwrap())
            .is_ok()
    );

    // Bob lost: no bonus, but his day has begun.
    let bob = &report.players[1];
    assert!(!bob.reward.daily_bonus);
    assert_eq!(bob.full.daily, lobby::BonusBlock::default());
    let bob = account(&store, 11);
    assert_eq!(
        (bob.extras.bonus.day, crate::v026g::bonus::progress(&bob)),
        (1, 0)
    );

    // The second and the third victory too; the fourth is a plain one.
    for win in 2..=4 {
        let mut next = result(players.clone());
        next.match_id = MATCH + win;
        let report = results
            .settle(&next, &store, &terms(), &mut Rng::new(1))
            .unwrap();
        assert_eq!(report.players[0].reward.daily_bonus, win <= 3, "win {win}");
    }
    assert_eq!(crate::v026g::bonus::progress(&account(&store, 10)), 3);

    // One who left before the end is not counted, whatever its team did.
    let store = accounts(&players);
    store.get(10).unwrap().locked().extras.bonus = Default::default();
    let mut left = players.clone();
    left[0].present_at_end = false;
    left[0].left = true;
    let report = Results::new()
        .settle(&result(left), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert!(!report.players[0].reward.daily_bonus);
    assert_eq!(account(&store, 10).extras.bonus.day, 0);
}

#[test]
fn the_third_victory_of_the_seventh_day_gives_the_weeks_reward() {
    let players = alice_and_bob();
    let store = accounts(&players);
    {
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        // Six days made, and two victories today.
        alice.extras.bonus.day = 7;
        alice.extras.bonus.wins = [3, 3, 3, 3, 3, 3, 2];
        // Past level 1, far from level 2: the match pays no level's gold.
        alice.exp_total = 600;
    }
    let before = account(&store, 10);
    let results = Results::new();
    let report = results
        .settle(&result(players.clone()), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    assert!(alice.reward.daily_bonus);
    assert_eq!(
        alice.reward.daily_reward,
        Some(crate::v026g::bonus::Given {
            dict_id: PREMIUM_MONEY,
            count: 50,
            item_id: 0
        })
    );
    let after = account(&store, 10);
    let (_, gold) = dropped_money(&alice.reward);
    assert_eq!(after.premium_money, before.premium_money + 50 + gold);
    assert!(after.extras.bonus.completed);
    // The window's message says so: seven days, each made, the reward
    // given.
    let message = crate::v026g::bonus::message(&after, false)
        .unwrap()
        .unwrap();
    assert_eq!(message.body()[..8], [7, 3, 3, 3, 3, 3, 3, 3]);
    let mut client = Client::signed_in();
    assert!(matches!(
        client.receive(&message).unwrap(),
        Decoded::DailyBonus {
            completed: true,
            ..
        }
    ));
    // Once.
    let mut next = result(players);
    next.match_id = MATCH + 1;
    let report = results
        .settle(&next, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert_eq!(report.players[0].reward.daily_reward, None);
    assert!(!report.players[0].reward.daily_bonus);
}

#[test]
fn a_level_reached_is_told_with_its_reward() {
    let players = alice_and_bob();
    let store = accounts(&players);
    // One point short of level 1.
    store.get(10).unwrap().locked().exp_total = 499;
    let gold = account(&store, 10).premium_money;
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    assert_eq!(alice.reward.levels, [1]);
    assert_eq!(alice.reward.level_gold, 100);
    // Important message type 14: the level, the money's kind twice (gold)
    // and the amount.
    assert!(alice.notices.contains(&(14, "[1][1][1][100]".to_owned())));
    assert!(account(&store, 10).premium_money >= gold + 100);
    // Bob reached none.
    assert!(
        report.players[1]
            .notices
            .iter()
            .all(|notice| notice.0 != 14)
    );
    // The client takes it as any important message.
    let mut client = Client::signed_in();
    let message = lobby::important_message(8, 0, "", "", 14, "[1][1][1][100]", 77);
    assert!(matches!(
        client.receive(&message).unwrap(),
        Decoded::Important { kind: 14, .. }
    ));
}

#[test]
fn a_kit_change_before_play_is_a_life_more_for_the_supplies_used() {
    use survarium::v026g::match_msgs::players::{ItemLeft, Supplies};
    let medkit = Dictionary::shared().item_named("medkit").unwrap().dict_id;
    // Alice died once and her block says none of her 5 medkits is left;
    // a life takes 2 out of the store.
    let mut players = alice_and_bob();
    players[0].supplies = Some(Supplies {
        alive: false,
        kit: 0,
        inventory_read: false,
        items: vec![ItemLeft {
            dict_id: medkit,
            stack: true,
            stored: 0,
            in_hand: None,
            condition: None,
            magazine: None,
            slot: None,
        }],
    });
    let used = |kit_changes: u16| {
        let mut players = players.clone();
        players[0].kit_changes = kit_changes;
        let store = accounts(&players);
        {
            let alice = store.get(10).unwrap();
            let mut alice = alice.locked();
            let kits = alice.add_item(medkit, 5);
            alice.kits[0].slots[13] = Some(crate::v026g::KitSlot {
                item_id: kits,
                count: 2,
            });
        }
        let report = Results::new()
            .settle(&result(players), &store, &terms(), &mut Rng::new(1))
            .unwrap();
        report.players[0].reward.supplies_used.clone()
    };
    // Two lives (one death): at most 4.
    assert_eq!(used(0), [(medkit, 4)]);
    // A kit change before play began, a respawn in place with the new
    // kit's medkits (patch 31): three lives, and all 5 were used.
    assert_eq!(used(1), [(medkit, 5)]);
}

#[test]
fn the_supplies_used_are_taken_and_the_boxes_put_them_right() {
    use survarium::v026g::match_msgs::players::{ItemLeft, Supplies};
    let dictionary = Dictionary::shared();
    let rounds = dictionary.item_named("ammo_7.62x25").unwrap().dict_id;
    let medkit = dictionary.item_named("medkit").unwrap().dict_id;
    let stack = |dict_id: u16, stored: u32| ItemLeft {
        dict_id,
        stack: true,
        stored,
        in_hand: None,
        condition: None,
        magazine: None,
        slot: None,
    };
    let mut players = alice_and_bob();
    // Alice fired 20 bullets and died once; her block, read when she was
    // dead, says 30 of her 50 rounds and 3 of her 5 medkits are left.
    players[0].statistics.as_mut().unwrap().events[event::BULLET_FIRE][0] = 20;
    players[0].supplies = Some(Supplies {
        alive: false,
        kit: 0,
        inventory_read: false,
        items: vec![stack(rounds, 30), stack(medkit, 3)],
    });
    let prepare = |store: &GameAccounts, boxes: bool| {
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        let kits = alice.add_item(medkit, 5);
        alice.kits[0].slots[13] = Some(crate::v026g::KitSlot {
            item_id: kits,
            count: 2,
        });
        alice.autobuy = boxes;
        alice.extras.auto_repair = boxes;
        alice.generic_money = 10_000;
        // The rank the factions sell their consumables at.
        for faction in 1..=4 {
            alice.add_reputation(faction, 60_000);
        }
        (alice.kits[0].slots[8].unwrap().item_id, kits)
    };

    // With neither box: the stacks shrink and nothing is spent.
    let store = accounts(&players);
    let (ammo, kits) = prepare(&store, false);
    let report = Results::new()
        .settle(&result(players.clone()), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let reward = &report.players[0].reward;
    assert_eq!(reward.supplies_used, [(rounds, 20), (medkit, 2)]);
    assert_eq!(reward.spent, lobby::SpentMoney::default());
    let alice = account(&store, 10);
    assert_eq!(alice.item(ammo).unwrap().condition_or_stack, 30);
    assert_eq!(alice.item(kits).unwrap().condition_or_stack, 3);
    assert!(report.spent_inventory(10, &alice).is_none());
    // Bob's block is not known and he fired nothing: nothing is taken.
    assert!(report.players[1].reward.supplies_used.is_empty());

    // With both: the medkits are bought back (the pistol's ammo is free),
    // the pistol the match wore is repaired, and the screen is told.
    let store = accounts(&players);
    let (ammo, kits) = prepare(&store, true);
    let report = Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let reward = &report.players[0].reward;
    assert_eq!(reward.supplies_used, [(rounds, 20), (medkit, 2)]);
    let price = survarium::v026g::prices::price(medkit).unwrap().cost;
    assert_eq!(reward.spent.autobuy_consumables, [(2 * price, medkit)]);
    assert!(reward.spent.autobuy_ammo.is_empty());
    let alice = account(&store, 10);
    assert_eq!(alice.item(kits).unwrap().condition_or_stack, 5);
    // The free ammo comes back by the clip.
    assert!(alice.item(ammo).unwrap().condition_or_stack >= 50);
    let pistol = alice.kits[0].slots[7].unwrap().item_id;
    assert_eq!(
        alice.item(pistol).unwrap().condition_or_stack,
        FULL_CONDITION
    );
    // The TT-33 costs nothing in the game's data, so its repair is free
    // and has no row.
    assert!(reward.spent.repair.is_empty());

    // The pushes: the inventory first, with the lists the client reads.
    let push = report
        .lobby_push(10, &alice, NOW, Some(1))
        .unwrap()
        .unwrap();
    let mut client = Client::signed_in();
    let read = client.receive_all(&push).unwrap();
    let Decoded::Inventory { spent, .. } = &read[0] else {
        panic!("the inventory goes first");
    };
    assert_eq!(*spent, reward.spent);
    // For a client that signed in after the match: the inventory again,
    // before the screen.
    let again = report.spent_inventory(10, &alice).unwrap().unwrap();
    assert_eq!(again, push[0]);
}

#[test]
fn a_report_whose_screen_may_be_open_is_kept_past_the_last_ones() {
    let players = alice_and_bob();
    let store = accounts(&players);
    let results = Results::new();
    results
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    // Alice is shown the screen; Bob is away.
    assert_eq!(results.take_unseen(10).unwrap().report.match_id, MATCH);
    // More matches than are kept end while her screen is open.
    for index in 0..KEPT as u32 + 5 {
        let mut next = result(alice_and_bob());
        next.match_id = 0x1000 + index;
        results.settle(&next, &store, &terms(), &mut Rng::new(3));
    }
    // A click on a row of her screen is still answered (`3D 00`).
    let found = results.find(MATCH).expect("the report of an open screen");
    assert!(found.player(10).is_some());
    assert!(found.player_details(2).unwrap().is_ok());
    // The others went as before: the last ones are kept.
    assert!(results.find(0x1000).is_none());
    assert!(results.find(0x1000 + 4).is_none());
    assert!(results.find(0x1000 + 5).is_some());
    assert_eq!(results.inner.locked().reports.len(), KEPT + 1);

    // A newer screen takes its place (the answer to `41 00` counts too).
    results.shown(10, 0x1000 + 5);
    let mut next = result(alice_and_bob());
    next.match_id = 0x2000;
    results.settle(&next, &store, &terms(), &mut Rng::new(3));
    assert!(results.find(MATCH).is_none());
    assert!(results.find(0x1000 + 5).is_some());
    assert_eq!(results.inner.locked().reports.len(), KEPT + 1);

    // And a screen is not waited for for ever.
    let mut inner = results.inner.locked();
    inner.make_room(quests::now() + SHOWN_FOR_SECONDS + 1);
    assert!(inner.shown.is_empty());
    assert!(inner.find(0x1000 + 5).is_none());
    assert_eq!(inner.reports.len(), KEPT - 1);
}

// ---- test bots (`bots`, `SRP_BOT_ACCOUNTS`) ----

/// A match of one player against the server's bots ("Player vs Bots"):
/// its silver and experience at `SRP_PVB_PAY` (half), no quest counted and
/// no daily bonus, no rating moved. Bob is one of the server's bots.
#[test]
fn a_match_against_bots_pays_half_and_counts_no_quest() {
    let mut players = alice_and_bob();
    players[1].driven = true;
    players[1].account_id = 0;
    players[1].session_id = 0;
    let mut outcome = result(players.clone());
    outcome.trusted_by = 1;
    let quest = Quests::shared().quest(10101).unwrap();
    // Alice has not played today, takes the Scavengers' quests and holds
    // their first (win with the base not robbed), which this match meets.
    let fresh = || {
        let store = accounts(&players[..1]);
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        alice.extras.bonus = Default::default();
        quests::choose_faction(&mut alice, 1).unwrap();
        alice.quests = vec![QuestInstance {
            instance_id: 50,
            quest_id: quest.id,
            games: 0,
            expires_at: NOW + 10_000,
        }];
        alice.next_quest_instance = 51;
        drop(alice);
        store
    };

    // The same match as any other: the quest is paid, the day's first
    // victory doubled.
    let store = fresh();
    let plain = Results::new()
        .settle(&outcome, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let plain = &plain.players[0].reward;
    assert!(plain.daily_bonus && plain.quest_money >= quest.money);

    // Against bots: no daily bonus, the quest held as it was.
    outcome.vs_bots = true;
    let store = fresh();
    let report = Results::new()
        .settle(&outcome, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    assert!(alice.reward.won && !alice.reward.daily_bonus);
    assert_eq!(alice.reward.daily_reward, None);
    assert_eq!(alice.reward.quest_money, 0);
    assert!(alice.reward.quest_reputation.is_empty());
    assert_eq!(alice.full.quest_money, 0);
    assert!(!alice.notices.iter().any(|(kind, _)| *kind == QUEST_MESSAGE));
    let after = account(&store, 10);
    assert_eq!(after.extras.bonus, Default::default());
    assert_eq!(after.quests.len(), 1);
    assert_eq!(
        (after.quests[0].quest_id, after.quests[0].games),
        (10101, 0)
    );
    // No rating moves (a bot of the server's is in it).
    assert_eq!(alice.reward.rating_after, alice.reward.rating_before);
    assert_eq!(after.rating, pay::START_RATING);
    assert_eq!(after.balance, Default::default());

    // Half the silver and experience: at the money rate 100 and the
    // experience rate 2 it pays what a match not against bots pays at 50
    // and 1 (with the day's bonus spent and no quest), cell by cell.
    // Reputation is whole.
    let store = accounts(&players[..1]);
    quests::choose_faction(&mut store.get(10).unwrap().locked(), 1).unwrap();
    let half = Terms {
        money_rate: pay::MoneyRate::Flat(50),
        ..terms()
    };
    let mut reference = outcome.clone();
    reference.vs_bots = false;
    let reference = Results::new()
        .settle(&reference, &store, &half, &mut Rng::new(1))
        .unwrap();
    let reference = &reference.players[0];
    let whole = Terms {
        experience_rate: 2,
        ..terms()
    };
    let store = fresh();
    let report = Results::new()
        .settle(&outcome, &store, &whole, &mut Rng::new(1))
        .unwrap();
    let alice = &report.players[0];
    assert_eq!(alice.reward.money, reference.reward.money);
    assert!(alice.reward.money > 0);
    assert_eq!(alice.reward.reputation, reference.reward.reputation);
    assert_eq!(alice.full.components, reference.full.components);
    assert_eq!(alice.full.premium, reference.full.premium);
    assert_eq!(alice.reward.experience, reference.reward.experience);
    assert!(alice.reward.experience > 0);
    // At the default half and rate 1, less than the plain match's.
    let store = fresh();
    let report = Results::new()
        .settle(&outcome, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert!(report.players[0].reward.experience < plain.experience);
    assert!(report.players[0].reward.money < plain.money);
    // The screen of the player still comes out.
    assert!(report.screen_for(10).is_some());
}

/// Alice and Bob, and Carol, who came late (two minutes before the end)
/// and stayed: each with the Scavengers' first quest (win with the base not
/// robbed), which the match meets for Alice and Carol, and the day's first
/// victory to come.
fn private_room_players() -> (Vec<PlayerResult>, impl Fn() -> GameAccounts) {
    let mut players = alice_and_bob();
    let mut carol = player(2, 0, "Carol", counters_of(0, &[(event::KILL, 1)]));
    carol.time_in_match_ms = 120_000;
    players.push(carol);
    let fresh = {
        let players = players.clone();
        move || {
            let store = accounts(&players);
            for player in &players {
                let account = store.get(player.account_id).unwrap();
                let mut account = account.locked();
                account.extras.bonus = Default::default();
                quests::choose_faction(&mut account, 1).unwrap();
                account.quests = vec![QuestInstance {
                    instance_id: 50,
                    quest_id: 10101,
                    games: 0,
                    expires_at: NOW + 10_000,
                }];
                account.next_quest_instance = 51;
            }
            store
        }
    };
    (players, fresh)
}

/// A room with a PIN's match pays nobody in it anything, the late joiner
/// neither, and takes nothing (the owner's of 2026-10-20): no experience,
/// silver, reputation, quest, daily bonus, drop or rating, no wear and no
/// supplies; every account is as it was. Its screen still comes out with
/// the match's numbers. The same match of a room without a PIN pays them.
#[test]
fn a_private_rooms_match_pays_nobody_anything() {
    let (players, fresh) = private_room_players();
    let quest = Quests::shared().quest(10101).unwrap();

    let store = fresh();
    let plain = Results::new()
        .settle(&result(players.clone()), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert!(!plain.private);
    for player in &plain.players {
        let reward = &player.reward;
        assert!(reward.experience > 0 && reward.money > 0, "{reward:?}");
        assert!(reward.reputation > 0 && reward.drop_chance > 0.0);
        assert_ne!(reward.rating_after, reward.rating_before);
    }
    assert!(plain.players[0].reward.quest_money >= quest.money);
    assert!(plain.players[0].reward.daily_bonus);
    assert!(!plain.players[0].reward.worn.is_empty());

    let mut private = result(players);
    private.private = true;
    let store = fresh();
    let before: Vec<Account> = (10..=12).map(|id| account(&store, id)).collect();
    let report = Results::new()
        .settle(&private, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert!(report.private);
    assert_eq!(report.players.len(), 3);
    for (player, before) in report.players.iter().zip(&before) {
        let reward = &player.reward;
        assert_eq!(reward.standing, Standing::Finished);
        assert_eq!(
            (
                reward.experience,
                reward.money,
                reward.reputation,
                reward.quest_money
            ),
            (0, 0, 0, 0),
            "{reward:?}"
        );
        assert_eq!(reward.premium_bonus, (0, 0, 0));
        assert!(reward.quest_reputation.is_empty() && reward.levels.is_empty());
        assert_eq!((reward.level_money, reward.level_gold), (0, 0));
        assert_eq!((reward.drop, reward.drop_chance), (None, 0.0));
        assert!(!reward.daily_bonus && reward.daily_reward.is_none());
        assert_eq!(reward.rating_after, reward.rating_before);
        assert!(reward.worn.is_empty() && reward.defects.is_empty());
        assert!(reward.supplies_used.is_empty());
        assert!(player.notices.is_empty());
        assert_eq!(player.full.quest_money, 0);
        assert_eq!(player.full.dropped, None);
        assert_eq!(player.full.drop_chances, [0.0; 4]);
        assert_eq!(account(&store, player.account_id), *before);
    }
    // The screen: who won, the rows and their scores; nothing paid.
    assert_eq!(report.stats.winner_team, 0);
    assert_eq!(report.stats.rows.len(), 3);
    assert!(report.stats.rows[0].score > 0);
    assert_eq!(
        report.players[0].reward.score,
        plain.players[0].reward.score
    );
    for id in 10..=12 {
        assert!(report.screen_for(id).is_some());
    }
}

/// A room with a PIN's match takes no drop card ("private match"); a
/// match of a room without one does.
#[test]
fn a_private_rooms_match_takes_no_drop_card() {
    let players = alice_and_bob();
    let store = accounts(&players);
    let results = Results::new();
    let card = Dictionary::shared().item_named(DROP_CARD).unwrap().dict_id;
    let cards = store.get(10).unwrap().locked().add_item(card, 3);
    let mut private = result(players.clone());
    private.private = true;
    results
        .settle(&private, &store, &terms(), &mut Rng::new(3))
        .unwrap();
    let mut rng = Rng::new(5);
    assert_eq!(
        use_card_of(&results, &store, MATCH, 10, cards, &mut rng),
        Err(DropCardRefusal::Private)
    );
    assert_eq!(DropCardRefusal::Private.to_string(), "private match");
    // One not in the match is refused as that, private or not.
    let erin_cards = store.get_or_create(14, "Erin").locked().add_item(card, 3);
    assert_eq!(
        use_card_of(&results, &store, MATCH, 14, erin_cards, &mut rng),
        Err(DropCardRefusal::NotAPlayer)
    );

    let mut plain = result(players);
    plain.match_id = MATCH + 1;
    results
        .settle(&plain, &store, &terms(), &mut Rng::new(3))
        .unwrap();
    assert_eq!(
        use_card_of(&results, &store, MATCH + 1, 10, cards, &mut rng).map(|(_, left)| left),
        Ok(2)
    );
}

/// A report kept before rooms with a PIN paid nothing has no `private`:
/// it is read as a match that paid. One kept before matches against bots
/// were marked has no `vs_bots`: it is read as PvP.
#[test]
fn a_report_kept_without_the_private_mark_is_not_private() {
    let players = alice_and_bob();
    let store = accounts(&players);
    let mut private = result(players);
    private.private = true;
    private.vs_bots = true;
    let report = Results::new()
        .settle(&private, &store, &terms(), &mut Rng::new(3))
        .unwrap();
    let mut body = serde_json::to_value(&*report).unwrap();
    body.as_object_mut().unwrap().remove("private").unwrap();
    body.as_object_mut().unwrap().remove("vs_bots").unwrap();
    let rows = vec![(report_key(MATCH), body.to_string())];
    let [kept] = &kept_reports(&rows)[..] else {
        panic!("one report kept");
    };
    assert!(!kept.private);
    assert!(!kept.vs_bots);
    assert_eq!(kept.match_id, MATCH);
}

#[test]
fn a_match_with_a_test_bot_moves_no_rating_and_pays_the_bot_no_daily_bonus() {
    // Bob is a bot, and wins: neither has played today.
    let mut players = alice_and_bob();
    players[1].statistics = None;
    players[1].score = 0;
    let mut outcome = result(players.clone());
    outcome.winner = Some(1);
    outcome.trusted_by = 1;
    let fresh = |store: &GameAccounts| {
        for id in [10, 11] {
            store.get(id).unwrap().locked().extras.bonus = Default::default();
        }
    };

    // With the setting off Bob is a player as any other: the day's first
    // victory is doubled for him, and the ratings move.
    let store = accounts(&players);
    fresh(&store);
    let plain = Results::new()
        .settle(&outcome, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert!(plain.players[1].reward.daily_bonus);
    assert_ne!(
        plain.players[0].reward.rating_after,
        plain.players[0].reward.rating_before
    );
    assert_ne!(account(&store, 11).extras.bonus, Default::default());

    // Named as a bot: no daily bonus and no run of days for him, nobody's
    // rating moves, and the rest is settled as for any account (a bot has
    // no counters: it is paid for staying and for the win).
    let store = accounts(&players);
    fresh(&store);
    store.set_bots(&["bob".to_owned()]);
    let report = Results::new()
        .settle(&outcome, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let (alice, bob) = (&report.players[0], &report.players[1]);
    assert!(bob.reward.won && !bob.reward.daily_bonus);
    assert_eq!(bob.reward.daily_reward, None);
    assert_eq!(account(&store, 11).extras.bonus, Default::default());
    assert!(bob.reward.experience > 0 && bob.reward.money > 0);
    assert!(bob.reward.experience < plain.players[1].reward.experience);
    for player in [alice, bob] {
        assert_eq!(player.reward.rating_after, player.reward.rating_before);
    }
    assert_eq!(account(&store, 10).rating, pay::START_RATING);
    assert_eq!(account(&store, 11).rating, pay::START_RATING);
    // Alice, a real player, is paid what she was without the setting.
    assert_eq!(alice.reward.experience, plain.players[0].reward.experience);
    assert_eq!(alice.reward.money, plain.players[0].reward.money);
    // The results screen of each still comes out.
    for id in [10, 11] {
        assert!(report.screen_for(id).is_some());
    }
}

#[test]
fn a_match_with_a_server_bot_moves_no_rating_and_leaves_nothing_for_account_0() {
    // Bob's place is the match server's own bot (as against bots): no
    // account, no session.
    let mut players = alice_and_bob();
    let store = accounts(&players);
    players[1].driven = true;
    (players[1].session_id, players[1].account_id) = (0, 0);
    players[1].statistics = None;
    players[1].score = 0;
    let mut outcome = result(players);
    outcome.winner = Some(0);
    outcome.trusted_by = 1;
    let results = Results::new();
    let report = results
        .settle(&outcome, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    // Alice is paid for her win, and her rating stays: a match against a
    // bot says nothing of her.
    let alice = &report.players[0];
    assert!(alice.reward.won && alice.reward.money > 0);
    assert_eq!(alice.reward.rating_after, alice.reward.rating_before);
    assert_eq!(account(&store, 10).rating, pay::START_RATING);
    assert!(report.screen_for(10).is_some());
    // Nothing is kept for an account 0 to be shown.
    assert!(!results.has_unseen(0));
}

/// The players' accounts ([`accounts`]) kept in `repository`.
fn kept_accounts(players: &[PlayerResult], repository: &MemoryGameRepository) -> GameAccounts {
    let store = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
    for player in players {
        let account = store.get_or_create(player.account_id, &player.name);
        let mut account = account.locked();
        if account.exp_total == 0 {
            only_the_pistol(&mut account);
            account.exp_total = PLAYED;
            account.extras.bonus = spent_day();
        }
    }
    store
}

fn at(now: u64) -> Terms {
    Terms { now, ..terms() }
}

fn match_of(match_id: u32) -> MatchResult {
    MatchResult {
        match_id,
        ..result(alice_and_bob())
    }
}

/// An update of the server restarts srp. Until 2026-10-12 that forgot the
/// results screens not shown yet (with their notices and the premium bonus
/// to be claimed from them) and started every account's play period anew.
#[test]
fn a_restart_keeps_the_screens_not_shown_and_the_play_periods() {
    let players = alice_and_bob();
    let repository = MemoryGameRepository::default();
    // The same matches with no restart in between, to compare with.
    let plain_store = accounts(&players);
    let plain = Arc::new(Results::new());

    let store = kept_accounts(&players, &repository);
    quests::choose_faction(&mut store.get(10).unwrap().locked(), 1).unwrap();
    quests::choose_faction(&mut plain_store.get(10).unwrap().locked(), 1).unwrap();
    let results = Results::attached(&store);
    for (index, now) in [NOW, NOW + 900].into_iter().enumerate() {
        let result = match_of(MATCH + index as u32);
        let seed = 5 + index as u64;
        results
            .settle(&result, &store, &at(now), &mut Rng::new(seed))
            .unwrap();
        plain
            .settle(&result, &plain_store, &at(now), &mut Rng::new(seed))
            .unwrap();
        // The lobby flushes after a match.
        store.flush();
    }
    // Bob is shown his screen, buys premium and claims the first match's
    // half; Alice never came back to the lobby.
    for results in [&results, &plain] {
        assert_eq!(results.take_unseen(11).unwrap().report.match_id, MATCH + 1);
    }
    for (results, store) in [(&results, &store), (&plain, &plain_store)] {
        let bob = store.get(11).unwrap();
        let mut bob = bob.locked();
        bob.premium_until = NOW + 5000;
        assert!(
            results
                .claim_premium(MATCH, 11, &mut bob, NOW + 950)
                .is_some()
        );
    }
    store.mark_dirty(11);
    store.flush();
    let first = results.find(MATCH).unwrap();
    let second = results.find(MATCH + 1).unwrap();
    assert!(!first.notices_for(10).is_empty());
    drop(results);
    drop(store);

    // srp starts again on the same database.
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    // The reports are what they were, to the last number.
    assert_eq!(results.find(MATCH).unwrap(), first);
    assert_eq!(results.find(MATCH + 1).unwrap(), second);
    assert!(results.find(MATCH + 2).is_none());
    // A row of the screen can be asked for, as before.
    let mut client = Client::signed_in();
    client
        .receive(
            &results
                .find(MATCH)
                .unwrap()
                .screen_for(10)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    client
        .receive(
            &results
                .find(MATCH)
                .unwrap()
                .player_details(2)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    // Bob saw his; what he claimed is not paid twice, what he did not is
    // still his to claim.
    assert!(!results.has_unseen(11));
    {
        let bob = store.get(11).unwrap();
        let mut bob = bob.locked();
        assert!(bob.is_premium(NOW + 960));
        assert_eq!(results.claim_premium(MATCH, 11, &mut bob, NOW + 960), None);
        let plain_bob = plain_store.get(11).unwrap();
        let mut plain_bob = plain_bob.locked();
        assert_eq!(
            results.claim_premium(MATCH + 1, 11, &mut bob, NOW + 960),
            plain.claim_premium(MATCH + 1, 11, &mut plain_bob, NOW + 960)
        );
        assert!(
            results
                .claim_premium(MATCH + 1, 11, &mut bob, NOW + 960)
                .is_none()
        );
    }
    store.mark_dirty(11);
    // Alice's third match of the period is her third: the report, with
    // the drop chance's play time part, is the one of no restart.
    let third = match_of(MATCH + 2);
    let kept = results
        .settle(&third, &store, &at(NOW + 1800), &mut Rng::new(7))
        .unwrap();
    let expected = plain
        .settle(&third, &plain_store, &at(NOW + 1800), &mut Rng::new(7))
        .unwrap();
    assert_eq!(
        kept.players[0].full.drop_chances,
        expected.players[0].full.drop_chances
    );
    assert_eq!(kept.players[0].reward, expected.players[0].reward);
    // She is shown the last screen and the notices of all three.
    assert!(results.has_unseen(10));
    let unseen = results.take_unseen(10).unwrap();
    let plain_unseen = plain.take_unseen(10).unwrap();
    assert_eq!(unseen.report.match_id, MATCH + 2);
    assert_eq!(unseen.notices, plain_unseen.notices);
    assert!(unseen.notices.len() >= first.notices_for(10).len());
    store.flush();
    drop(results);
    drop(store);

    // And once more: nothing is shown twice.
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    assert!(!results.has_unseen(10));
    assert!(results.take_unseen(10).is_none());
    assert_eq!(results.take_unseen(11).unwrap().report.match_id, MATCH + 2);
    assert_eq!(results.find(MATCH + 2).unwrap(), kept);
}

/// The play period's counter is what a restart reset until 2026-10-12: a
/// player's drop chance went back to its first game's at every update.
#[test]
fn the_game_of_the_play_period_is_counted_across_a_restart() {
    let players = alice_and_bob();
    let chance = |restart_after: Option<usize>| {
        let repository = MemoryGameRepository::default();
        let mut store = kept_accounts(&players, &repository);
        let mut results = Results::attached(&store);
        let mut chances = Vec::new();
        for game in 0..4 {
            let report = results
                .settle(
                    &match_of(MATCH + game as u32),
                    &store,
                    &at(NOW + 900 * game as u64),
                    &mut Rng::new(1),
                )
                .unwrap();
            chances.push((report.players[0].full.drop_chances[0] * 1000.0).round() as u32);
            store.flush();
            if restart_after == Some(game) {
                store = kept_accounts(&players, &repository);
                results = Results::attached(&store);
            }
        }
        chances
    };
    let straight = chance(None);
    // The chance moves with the game of the period, so the test tells.
    assert_ne!(straight[0], straight[3]);
    assert_eq!(chance(Some(0)), straight);
    assert_eq!(chance(Some(2)), straight);
}

#[test]
fn only_the_reports_kept_stay_in_the_repository() {
    use crate::v026g::persist::GameRepository;
    let players = alice_and_bob();
    let mut repository = MemoryGameRepository::default();
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    for index in 0..KEPT as u32 + 3 {
        results
            .settle(
                &match_of(0x1000 + index),
                &store,
                &at(NOW),
                &mut Rng::new(3),
            )
            .unwrap();
        // Not after every match: several changes go in one pass.
        if index % 5 == 0 {
            store.flush();
        }
    }
    store.flush();
    let rows = repository.load_results().unwrap();
    let reports: Vec<&str> = rows
        .iter()
        .filter_map(|(key, _)| key.strip_prefix(REPORT_KEY))
        .collect();
    assert_eq!(reports.len(), KEPT);
    assert!(!reports.contains(&"4096"));
    assert!(reports.contains(&(0x1000 + KEPT as u32 + 2).to_string().as_str()));
    assert_eq!(rows.len(), KEPT + 1);
    // Nothing changed: nothing is written.
    let before = rows.clone();
    store.flush();
    assert_eq!(repository.load_results().unwrap(), before);

    // The notices of a report that went are kept for who has not seen it.
    drop(results);
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    assert!(results.find(0x1000).is_none());
    assert_eq!(
        results.take_unseen(10).unwrap().report.match_id,
        0x1000 + KEPT as u32 + 2
    );
}

/// A kept screen that shows an item the game's data lacks (data made
/// without a patch) stops the load, named, and nothing is written.
#[test]
fn a_kept_report_of_an_item_the_data_lacks_stops_the_load() {
    use crate::v026g::persist::GameRepository;
    let players = alice_and_bob();
    let mut repository = MemoryGameRepository::default();
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    results
        .settle(&match_of(MATCH), &store, &at(NOW), &mut Rng::new(3))
        .unwrap();
    store.flush();
    drop(results);
    drop(store);
    let key = report_key(MATCH);
    let rows = repository.load_results().unwrap();
    let body = &rows.iter().find(|row| row.0 == key).unwrap().1;
    let mut report: MatchReport = serde_json::from_str(body).unwrap();
    report.players[1].reward.supplies_used.push((9998, 3));
    let body = serde_json::to_string(&report).unwrap();
    repository.save_result(&key, &body).unwrap();
    let saves = repository.saves();

    let error = GameAccounts::with_repository(Box::new(repository.clone()))
        .err()
        .expect("refused")
        .to_string();
    assert!(
        error.contains(&format!("item dict ids 9998 (report {MATCH:#x})")),
        "{error}"
    );
    assert_eq!(repository.saves(), saves);
}

/// Rows written by other code, or damaged: a screen is lost, nothing else.
#[test]
fn rows_that_do_not_read_are_left_out_and_cleaned_up() {
    use crate::v026g::persist::GameRepository;
    let players = alice_and_bob();
    let mut repository = MemoryGameRepository::default();
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    for index in 0..3 {
        results
            .settle(&match_of(MATCH + index), &store, &at(NOW), &mut Rng::new(3))
            .unwrap();
    }
    store.flush();
    drop(results);
    drop(store);
    let good = repository.load_results().unwrap();
    let body_of = |key: &str| good.iter().find(|row| row.0 == key).unwrap().1.clone();

    // One report damaged, one another match's, one nothing lists, and a
    // row that is nobody's.
    repository
        .save_result(&report_key(MATCH), "{\"match_id\":")
        .unwrap();
    repository
        .save_result(&report_key(MATCH + 1), &body_of(&report_key(MATCH + 2)))
        .unwrap();
    repository
        .save_result(&report_key(0x9999), &body_of(&report_key(MATCH + 2)))
        .unwrap();
    repository.save_result("other", "kept as it is").unwrap();
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    assert!(results.find(MATCH).is_none());
    assert!(results.find(MATCH + 1).is_none());
    assert!(results.find(0x9999).is_none());
    assert_eq!(results.find(MATCH + 2).unwrap().match_id, MATCH + 2);
    // Who had not seen them is shown the one that is left.
    assert_eq!(results.take_unseen(10).unwrap().report.match_id, MATCH + 2);
    store.flush();
    let keys: Vec<String> = repository
        .load_results()
        .unwrap()
        .into_iter()
        .map(|row| row.0)
        .collect();
    assert_eq!(
        keys,
        [
            "other".to_owned(),
            report_key(MATCH + 2),
            STATE_KEY.to_owned()
        ]
    );
    drop(results);
    drop(store);

    // A state that does not read: as a database of before.
    repository.save_result(STATE_KEY, "[]").unwrap();
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    assert!(results.find(MATCH + 2).is_none());
    assert!(!results.has_unseen(11));
    store.flush();
    let rows = repository.load_results().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|row| row.0 == STATE_KEY && row.1.starts_with('{'))
    );
}

/// Accounts kept in memory keep no results either, and nothing fails.
#[test]
fn results_attached_to_accounts_in_memory_are_plain_ones() {
    let players = alice_and_bob();
    let store = accounts(&players);
    let results = Results::attached(&store);
    results
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert_eq!(store.flush(), 0);
    assert!(results.has_unseen(10));
}

/// Three players of team 0 and two of team 1, all with counters.
fn three_and_two() -> Vec<PlayerResult> {
    [
        (0, "Alice"),
        (1, "Bob"),
        (0, "Carol"),
        (1, "Dave"),
        (0, "Erin"),
    ]
    .into_iter()
    .enumerate()
    .map(|(place, (team, name))| {
        player(
            place as u8,
            team,
            name,
            counters_of(0, &[(event::KILL, 1), (event::DEATH, 1)]),
        )
    })
    .collect()
}

#[test]
fn a_match_moves_the_balance_rating_by_the_sides_sizes() {
    use crate::v026g::balance::Balance;

    // Team 0, the three, wins: what was expected, so it moves little.
    let players = three_and_two();
    let store = accounts(&players);
    Results::new()
        .settle(&result(players.clone()), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let expected = account(&store, 10).balance;
    assert!(expected.rating > 0 && expected.matches == 1, "{expected:?}");
    assert_eq!(account(&store, 12).balance, expected);
    let loser = account(&store, 11).balance;
    assert_eq!(
        loser,
        Balance {
            rating: -expected.rating,
            matches: 1
        }
    );

    // The two win: an upset, and it moves much further.
    let store = accounts(&players);
    let mut upset = result(players.clone());
    upset.winner = Some(1);
    upset.team_points = [1, 3];
    Results::new()
        .settle(&upset, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    let won = account(&store, 11).balance;
    assert!(won.rating > 5 * expected.rating, "{won:?} {expected:?}");
    assert_eq!(account(&store, 10).balance.rating, -won.rating);
    // The ranking's rating is as it was before this was added: by the
    // teams' averages, whatever their sizes.
    assert_eq!(account(&store, 11).rating, 102);
    assert_eq!(account(&store, 10).rating, 98);
}

#[test]
fn the_balance_rating_is_not_moved_by_a_match_that_says_nothing() {
    use crate::v026g::balance::Balance;

    let players = three_and_two();
    let untouched =
        |store: &GameAccounts| (10..15).all(|id| account(store, id).balance == Balance::default());

    // A whole side left: the winner is who stayed.
    let store = accounts(&players);
    let mut deserted = result(players.clone());
    deserted.end = MatchEnd::Deserted;
    Results::new()
        .settle(&deserted, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert!(untouched(&store));
    // The ranking's rating still moves, as before.
    assert_ne!(account(&store, 10).rating, pay::START_RATING);

    // No trusted result.
    let store = accounts(&players);
    let mut unknown = result(players.clone());
    unknown.trusted_by = 0;
    unknown.trusted_snapshot_ms = None;
    Results::new()
        .settle(&unknown, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert!(untouched(&store));

    // A test bot in it.
    let store = accounts(&players);
    store.set_bots(&["dave".to_owned()]);
    Results::new()
        .settle(&result(players.clone()), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert!(untouched(&store));

    // A draw is a match: it counts, and with equal sides it moves nothing.
    let pair = alice_and_bob();
    let store = accounts(&pair);
    let mut draw = result(pair);
    draw.winner = Some(DRAW);
    draw.team_points = [2, 2];
    Results::new()
        .settle(&draw, &store, &terms(), &mut Rng::new(1))
        .unwrap();
    assert_eq!(
        account(&store, 10).balance,
        Balance {
            rating: 0,
            matches: 1
        }
    );
}

#[test]
fn a_match_of_few_players_moves_no_balance_rating() {
    use crate::v026g::balance::{self, Balance};

    // srp's own terms: a match moves the rating from four players.
    assert_eq!(Terms::at(NOW).rated_players, balance::RATED_PLAYERS);
    let own = Terms {
        rated_players: balance::RATED_PLAYERS,
        ..terms()
    };
    let settle = |players: Vec<PlayerResult>| {
        let store = accounts(&players);
        Results::new()
            .settle(&result(players), &store, &own, &mut Rng::new(1))
            .unwrap();
        store
    };
    let untouched = |store: &GameAccounts, ids: std::ops::Range<u32>| {
        ids.clone()
            .all(|id| account(store, id).balance == Balance::default())
    };

    // One against one: no rating moves, and it is nobody's match more.
    let duel = settle(alice_and_bob());
    assert!(untouched(&duel, 10..12));
    // The ranking's rating and the pay are as in any match.
    assert_ne!(account(&duel, 10).rating, pay::START_RATING);
    assert!(account(&duel, 10).exp_total > 0);

    // Two against one.
    let mut three = three_and_two();
    three.truncate(3);
    assert!(untouched(&settle(three), 10..13));

    // Three against two, but two of them came for the last tenth of it:
    // three players for most of it.
    let mut late = three_and_two();
    late[3].time_in_match_ms /= 10;
    late[4].time_in_match_ms /= 10;
    assert!(untouched(&settle(late), 10..15));

    // One of the five came late: four were in it, so it counts, and the
    // one who came late is moved too, by its share.
    let mut grown = three_and_two();
    grown[4].time_in_match_ms /= 10;
    let grown = settle(grown);
    let alice = account(&grown, 10).balance;
    let erin = account(&grown, 14).balance;
    assert!(alice.rating > 0 && alice.matches == 1, "{alice:?}");
    assert!(
        erin.rating > 0 && erin.rating < alice.rating && erin.matches == 1,
        "{erin:?}"
    );

    // Two against two from the start.
    let mut four = three_and_two();
    four.truncate(4);
    assert_eq!(account(&settle(four), 10).balance.matches, 1);
}

#[test]
fn one_who_left_or_never_got_in_and_the_balance_rating() {
    let mut players = three_and_two();
    // Erin (team 0) left before the end; Dave (team 1) never got in.
    players[4].present_at_end = false;
    players[4].left = true;
    players[3].entered = false;
    players[3].present_at_end = false;
    let store = accounts(&players);
    Results::new()
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();
    // Three against one, as it was played: Erin has her team's result and
    // nothing more, Dave is not counted and not moved.
    let alice = account(&store, 10).balance;
    assert!(alice.rating > 0);
    assert_eq!(account(&store, 14).balance, alice);
    assert_eq!(account(&store, 13).balance, Default::default());
    let bob = account(&store, 11).balance;
    assert_eq!((bob.rating < 0, bob.matches), (true, 1));
}

/// A release withdraws a scope pair after records of it were kept (round 4f
/// on the local test server, 2026-10-17: a kept results screen named the
/// variant 2432, Kedr + Docter, and srp refused to start). srp starts: the
/// account's record and the report's are taken off, and both are written
/// again; the scope item the player owns stays.
#[test]
fn a_withdrawn_scope_variant_in_the_kept_state_is_taken_off_at_start() {
    const WITHDRAWN: u32 = 2432;
    let players = alice_and_bob();
    let mut repository = MemoryGameRepository::default();
    let store = kept_accounts(&players, &repository);
    let results = Results::attached(&store);
    results
        .settle(&match_of(MATCH), &store, &at(NOW), &mut Rng::new(5))
        .unwrap();
    store.flush();
    drop(results);
    drop(store);
    assert!(Dictionary::shared().item(WITHDRAWN as u16).is_none());

    // The kept report's first item and Alice's weapon name the variant.
    let key = format!("{REPORT_KEY}{MATCH}");
    let rows = repository.load_results().unwrap();
    let body = &rows.iter().find(|(row, _)| *row == key).unwrap().1;
    let mut report: MatchReport = serde_json::from_str(body).unwrap();
    report.players[0].body.items[0]
        .upgrades
        .push(lobby::Upgrade {
            id: 9000,
            a: WITHDRAWN,
            value: 0,
            param: 3,
        });
    repository
        .save_result(&key, &serde_json::to_string(&report).unwrap())
        .unwrap();
    let alice_id = players[0].account_id;
    let mut alice = repository.get(alice_id).unwrap();
    let weapon = [7, 10]
        .into_iter()
        .find_map(|slot| alice.kits[0].slots[slot])
        .unwrap()
        .item_id;
    // 0.26g's Leupold stands for the shop scope the player owns.
    let scope = alice.add_item(69, FULL_CONDITION);
    let record = crate::v026g::account::Upgrade {
        id: scope,
        a: WITHDRAWN,
        value: 0,
        param: 3,
    };
    alice.item_mut(weapon).unwrap().upgrades.push(record);
    repository.save(alice_id, &alice).unwrap();

    // srp starts again: no refusal.
    let store = kept_accounts(&players, &repository);
    assert_eq!(
        items_rules::upgrades_taken_off(&alice, &store.get(alice_id).unwrap().locked()),
        [(weapon, record, "its dict id is no longer in the game data")]
    );
    {
        let account = store.get(alice_id).unwrap();
        let account = account.locked();
        assert!(account.item(weapon).unwrap().upgrades.is_empty());
        assert!(account.item(scope).is_some(), "the scope item stays");
    }
    assert!(store.is_dirty(alice_id));
    let results = Results::attached(&store);
    let kept = results.find(MATCH).unwrap();
    assert!(
        kept.players[0].body.items[0]
            .upgrades
            .iter()
            .all(|upgrade| upgrade.a != WITHDRAWN)
    );
    // Both are written again: the next start has nothing to take off.
    store.flush();
    assert!(repository.get(alice_id).unwrap().item(weapon).unwrap().upgrades.is_empty());
    let rows = repository.load_results().unwrap();
    let body = &rows.iter().find(|(row, _)| *row == key).unwrap().1;
    assert!(!body.contains(&format!("\"a\":{WITHDRAWN}")), "{body}");
    drop(results);
    let store = kept_accounts(&players, &repository);
    assert!(!store.is_dirty(alice_id));
}

/// Blackout (`docs/notes/blackout.md`): every point counts twice, in the
/// score and in what it pays; the counts on the screen are as played. The
/// same match without it pays once.
#[test]
fn a_blackout_match_pays_its_points_twice() {
    let players = alice_and_bob();
    let plain = result(players.clone());
    let mut blackout = result(players.clone());
    blackout.blackout = true;
    let once = Results::new()
        .settle(&plain, &accounts(&players), &terms(), &mut Rng::new(1))
        .unwrap();
    let twice = Results::new()
        .settle(&blackout, &accounts(&players), &terms(), &mut Rng::new(1))
        .unwrap();
    let (once, twice) = (&once.players[0], &twice.players[0]);
    assert_eq!((once.reward.score, twice.reward.score), (35, 70));
    // Kills 75 / 9, help 10 / 1, the fixed part of a win 82.73 / 122.69:
    // each cell twice before it is rounded down.
    assert_eq!(columns(&once.full, 0), (3.0, 75, 9, 0));
    assert_eq!(columns(&twice.full, 0), (3.0, 150, 18, 0));
    assert_eq!(columns(&once.full, 1), (1.0, 10, 1, 0));
    assert_eq!(columns(&twice.full, 1), (1.0, 20, 2, 0));
    assert_eq!(columns(&once.full, 2), (2.0, 82, 122, 0));
    assert_eq!(columns(&twice.full, 2), (2.0, 165, 245, 0));
    assert_eq!((once.reward.experience, once.reward.money), (132, 167));
    assert_eq!((twice.reward.experience, twice.reward.money), (265, 335));
}

#[test]
fn farming_limits_change_payment_without_changing_score_or_quests() {
    let mut p = player(
        0,
        0,
        "Alice",
        counters_of(0, &[(event::KILL, 23), (event::HEADSHOT_KILL, 12)]),
    );
    let original = result(vec![p.clone()]);
    p.pay_statistics = Some(counters_of(
        0,
        &[(event::KILL, 10), (event::HEADSHOT_KILL, 5)],
    ));
    let limited = result(vec![p]);
    let terms = terms();
    let before = Shared::of(&original, &terms);
    let after = Shared::of(&limited, &terms);
    assert!(after.rows_money(&limited.players[0], 0) < before.rows_money(&original.players[0], 0));
    assert_eq!(
        after.score(&limited.players[0]),
        before.score(&original.players[0])
    );
    assert_eq!(
        after.quest_stats(&limited.players[0]),
        before.quest_stats(&original.players[0])
    );
    let mut untrusted = limited;
    untrusted.trusted_by = 0;
    let shared = Shared::of(&untrusted, &terms);
    assert_eq!(shared.lines(&untrusted.players[0], 0)[0][0].0, 0.0);
}

#[test]
fn pve_rewards_stop_at_level_seven_including_the_money_rate_and_premium() {
    let players = alice_and_bob();
    let mut terms = terms();
    terms.money_rate = pay::MoneyRate::ByLevel;
    let settle = |level, vs_bots| {
        let mut outcome = result(players.clone());
        outcome.match_level = level;
        outcome.vs_bots = vs_bots;
        let store = accounts(&players);
        store.get(10).unwrap().locked().premium_until = terms.now + 3600;
        Results::new()
            .settle(&outcome, &store, &terms, &mut Rng::new(1))
            .unwrap()
    };
    let seven = settle(7, true);
    let fifteen = settle(15, true);
    for (low, high) in seven.players.iter().zip(&fifteen.players) {
        assert_eq!(high.full.components, low.full.components);
        assert_eq!(high.reward.money, low.reward.money);
        assert_eq!(high.reward.experience, low.reward.experience);
        assert_eq!(high.reward.reputation, low.reward.reputation);
        assert_eq!(high.full.premium, low.full.premium);
    }
    // PvP retains its normal level scaling; lower PvE levels stay lower.
    assert!(settle(15, false).players[0].reward.money > settle(7, false).players[0].reward.money);
    assert!(settle(3, true).players[0].reward.money < seven.players[0].reward.money);
}

#[test]
fn pve_drops_stop_at_level_seven() {
    let players = alice_and_bob();
    let settle = |level, vs_bots, seed| {
        let mut outcome = result(players.clone());
        outcome.match_level = level;
        outcome.vs_bots = vs_bots;
        Results::new()
            .settle(&outcome, &accounts(&players), &terms(), &mut Rng::new(seed))
            .unwrap()
            .players[0]
            .reward
            .drop
    };
    // The highest item level the generic table's row of match level 7 gives.
    let row = &Drops::shared().generic.levels[usize::from(crate::v026g::pvb::MAX_DROP_LEVEL)];
    let top = row.iter().rposition(|&weight| weight > 0).unwrap() as u8;
    let level_of = |drop: Option<Dropped>| {
        drop.filter(|drop| is_gear(drop.dict_id))
            .map(|drop| item_params(drop.dict_id).unwrap().item_level)
    };
    let (mut pve_gear, mut pvp_above) = (0, 0);
    for seed in 1..=300 {
        let at_seven = settle(7, true, seed);
        assert_eq!(settle(15, true, seed), at_seven, "seed {seed}");
        if let Some(level) = level_of(at_seven) {
            pve_gear += 1;
            assert!(level <= top, "seed {seed}: level {level} over {top}");
        }
        // PvP rolls at its own level.
        pvp_above += u32::from(level_of(settle(15, false, seed)).is_some_and(|level| level > top));
    }
    assert!(
        pve_gear > 30 && pvp_above > 30,
        "{pve_gear} PvE gear, {pvp_above} PvP above {top}, row {row:?}"
    );
}

#[test]
fn a_drop_card_on_a_pve_match_rolls_at_level_seven() {
    let card = Dictionary::shared().item_named(DROP_CARD).unwrap().dict_id;
    let players = alice_and_bob();
    // Alice's card on the screen of a match of `level`: the level it rolled
    // at and what it gave.
    let use_card = |level, vs_bots, seed| {
        let store = accounts(&players);
        let item_id = store.get(10).unwrap().locked().add_item(card, 3);
        let mut outcome = result(players.clone());
        outcome.match_level = level;
        outcome.vs_bots = vs_bots;
        let results = Results::new();
        results
            .settle(&outcome, &store, &terms(), &mut Rng::new(1))
            .unwrap();
        let alice = store.get(10).unwrap();
        let mut alice = alice.locked();
        let (rolled_at, used) = results
            .use_drop_card(MATCH, 10, &mut alice, item_id, &mut Rng::new(seed))
            .unwrap();
        (rolled_at, used.unwrap().dropped)
    };
    let row = &Drops::shared().drop_card.levels[usize::from(crate::v026g::pvb::MAX_DROP_LEVEL)];
    let top = row.iter().rposition(|&weight| weight > 0).unwrap() as u8;
    let level_of = |dropped: Dropped| {
        is_gear(dropped.dict_id).then(|| item_params(dropped.dict_id).unwrap().item_level)
    };
    let mut pvp_above = 0;
    for seed in 1..=100 {
        let (at, at_seven) = use_card(7, true, seed);
        assert_eq!(at, 7);
        assert_eq!(use_card(15, true, seed), (7, at_seven), "seed {seed}");
        assert!(
            level_of(at_seven).is_none_or(|level| level <= top),
            "seed {seed}"
        );
        // PvP rolls at its own level.
        let (at, pvp) = use_card(15, false, seed);
        assert_eq!(at, 15);
        pvp_above += u32::from(level_of(pvp).is_some_and(|level| level > top));
    }
    assert!(pvp_above > 10, "{pvp_above} PvP cards above {top}");
}

/// The history's records made so far and not written.
fn history_of(results: &Results) -> Vec<MatchRecord> {
    results.inner.locked().changes.history.clone()
}

#[test]
fn a_settled_match_is_kept_for_the_history_without_its_bots() {
    let mut players = alice_and_bob();
    // Carol (team 0) scored less than Alice and left; Dan never came in; a
    // server's bot played for team 1.
    let mut carol = player(
        2,
        0,
        "Carol",
        counters_of(1, &[(event::HEADSHOT_KILL, 2), (event::ARTEFACT, 1)]),
    );
    carol.present_at_end = false;
    carol.left = true;
    carol.score = 1;
    let mut dan = player(3, 1, "Dan", PlayerStatistics::default());
    dan.entered = false;
    dan.present_at_end = false;
    let mut bot = player(4, 1, "Bot", counters_of(0, &[(event::KILL, 9)]));
    bot.driven = true;
    bot.account_id = 0;
    players.extend([carol, dan, bot]);
    players[0].statistics = Some(counters_of(
        0,
        &[
            (event::KILL, 3),
            (event::ASSIST, 1),
            (event::DEATH, 1),
            (event::GRENADE_KILL, 1),
            (event::MELEE_KILL, 1),
            (event::PUT_VICTORY_ITEM_IN_CONTAINER, 2),
            (event::ARTEFACT, 2),
            // Patch 80's empty-slot bonus: not an artefact of its own.
            (event::WINNING_TEAM_VI_BONUS, 1),
        ],
    ));
    let store = accounts(&players);
    let results = Results::new();
    let report = results
        .settle(&result(players), &store, &terms(), &mut Rng::new(1))
        .unwrap();

    let records = history_of(&results);
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.game_match_id, MATCH);
    assert_eq!(record.kind, MatchKind::Public);
    assert_eq!(
        record.info,
        history::MatchInfo {
            ended_at: NOW,
            duration_ms: 700_000,
            map_id: 1,
            game_mode: mode::BATTERIES,
            match_level: 0,
            winner: history::Winner::FirstTeam,
            team_points: [3, 1],
        }
    );
    let ids: Vec<u32> = record
        .players
        .iter()
        .map(|player| player.account_id)
        .collect();
    assert_eq!(ids, [10, 11, 12], "no bot, nobody who never came in");
    let alice = &record.players[0].line;
    assert_eq!(
        (alice.team, alice.place, alice.presence),
        (0, 1, history::Presence::Finished)
    );
    assert_eq!((alice.kills, alice.deaths), (3, 1));
    assert_eq!(alice.rating, report.players[0].reward.rating_after);
    assert_eq!(
        alice.counters,
        history::Counters {
            assists: 1,
            headshots: 0,
            grenade_kills: 1,
            melee_kills: 1,
            batteries: 2,
            capture_hundredths: 0,
            artefacts: 2,
        }
    );
    let carol = &record.players[2].line;
    assert_eq!((carol.place, carol.presence), (2, history::Presence::Left));
    assert_eq!(
        (carol.counters.headshots, carol.counters.artefacts),
        (2, 1),
        "every kit counts"
    );
    assert_eq!(record.players[1].line.place, 1);
    assert!(record.players.iter().all(|player| player.clan_id.is_none()));
}

#[test]
fn a_private_room_and_a_match_against_bots_are_kept_by_their_kind() {
    for (private, vs_bots, kind) in [
        (true, false, MatchKind::Private),
        (false, true, MatchKind::VsBots),
        (true, true, MatchKind::Private),
    ] {
        let players = alice_and_bob();
        let store = accounts(&players);
        let results = Results::new();
        let mut result = result(players);
        result.private = private;
        result.vs_bots = vs_bots;
        results.settle(&result, &store, &terms(), &mut Rng::new(1));
        let records = history_of(&results);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, kind);
    }
}

#[test]
fn an_untrusted_match_is_kept_without_its_counters_or_winner() {
    let players = alice_and_bob();
    let store = accounts(&players);
    let results = Results::new();
    let mut result = result(players);
    result.trusted_by = 0;
    results.settle(&result, &store, &terms(), &mut Rng::new(1));
    let record = &history_of(&results)[0];
    assert_eq!(record.info.winner, history::Winner::Unknown);
    assert_eq!(
        record.players[0].line.counters,
        history::Counters::default()
    );
    assert_eq!(record.players[0].line.kills, 3, "the scoreboard's");

    // A match that never began has nobody in it: nothing is kept.
    let results = Results::new();
    let mut waited = alice_and_bob_result();
    waited.played_ms = None;
    results.settle(&waited, &store, &terms(), &mut Rng::new(1));
    assert!(history_of(&results).is_empty());
}

fn alice_and_bob_result() -> MatchResult {
    let mut result = result(alice_and_bob());
    result.end = MatchEnd::TimeLimit;
    result
}

#[test]
fn a_research_match_keeps_captures_not_batteries() {
    let mut players = alice_and_bob();
    players[0].statistics = Some(counters_of(
        0,
        &[
            (event::RESEARCH_CAPTURE, 150),
            (event::PUT_VICTORY_ITEM_IN_CONTAINER, 1),
        ],
    ));
    let store = accounts(&players);
    let results = Results::new();
    let mut result = result(players);
    result.game_mode = mode::RESEARCH;
    results.settle(&result, &store, &terms(), &mut Rng::new(1));
    let counters = history_of(&results)[0].players[0].line.counters;
    assert_eq!((counters.capture_hundredths, counters.batteries), (150, 0));
}

#[test]
fn the_history_is_written_at_the_next_save() {
    let dir = std::env::temp_dir().join(format!(
        "srp-g26-history-save-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let repository = crate::v026g::SqliteGameRepository::open_in(&dir).unwrap();
    let store = GameAccounts::with_repository(Box::new(repository)).unwrap();
    let players = alice_and_bob();
    for player in &players {
        store.get_or_create(player.account_id, &player.name);
    }
    let results = Results::attached(&store);
    results.settle(&result(players), &store, &terms(), &mut Rng::new(1));
    store.flush();
    assert!(history_of(&results).is_empty());
    let history = store.history().unwrap();
    let matches = history.player_matches(10, None, 10).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].1.kills, 3);
    assert_eq!(history.player(11).unwrap().unwrap().totals.matches, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

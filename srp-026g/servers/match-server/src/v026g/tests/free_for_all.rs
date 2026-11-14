//! Free for all (`docs/notes/ffa.md`): the registry gives every place a side
//! of its own and takes as many players as any match (20); a room's match of it
//! starts with who is in; it ends when a trusted world shows a player with
//! the kills that win, or at its time limit, and its result has no team
//! that won.

use super::*;
use crate::v026g::registry::LivePhase;
use crate::v026g::{CreateError, JoinError, MatchEnd};
use survarium::v026g::maps::{self, GameMode};
use survarium::v026g::match_msgs::MAX_PLAYERS;
use survarium::v026g::match_msgs::rules::state;
use survarium::v026g::match_msgs::static_info::parse_options;

/// The free-for-all on level_01 by day.
fn free_for_all() -> &'static maps::MapMode {
    maps::find(1, maps::weather::DAY, GameMode::FreeForAll).expect("level_01's free-for-all")
}

fn map_id() -> u8 {
    free_for_all().id
}

/// srp's entry after a free-for-all's options: tag 3, two bytes, the kills
/// that win.
fn srp_entry() -> Vec<u8> {
    let mut entry = b"SRPX\x03\x02".to_vec();
    entry.extend((free_for_all().kills_to_win as u16).to_le_bytes());
    entry
}

/// The lobby's seating of a match: its places on two sides, places 0 and 2
/// a squad, the places of `driven` the server's own bots.
fn seated(players: usize, map_id: u8, driven: &[usize]) -> MatchSetup {
    let mut made = setup(players, map_id);
    for place in [0, 2] {
        made.players[place].profile.squad = 1;
    }
    for &place in driven {
        let player = &mut made.players[place];
        (player.driven, player.session_id, player.account_id) = (true, 0, 0);
    }
    made
}

/// One who comes into a match being played, as the lobby seats it: on one
/// of two sides, in squad `squad`.
fn newcomer(place: usize, squad: u8) -> MatchPlayer {
    let mut profile = profile(&format!("late{place}"), (place % 2) as u8);
    profile.squad = squad;
    MatchPlayer {
        session_id: session_of(place),
        account_id: 100 + place as u32,
        profile,
        bot: false,
        driven: false,
    }
}

/// The teams and squads of a made match: its places', and those of the
/// options its clients are given.
fn sides(registry: &MatchRegistry, match_id: u32) -> (Vec<u8>, Vec<(u8, u8)>) {
    let formed = registry.formed(match_id).expect("the match");
    let options = parse_options(formed.info.options(), Dictionary::shared())
        .expect("options a client reads");
    assert_eq!(options.map, formed.played.match_map());
    let kept: Vec<(u8, u8)> = formed
        .profiles
        .iter()
        .map(|profile| (profile.team, profile.squad))
        .collect();
    let read: Vec<(u8, u8)> = options
        .players
        .iter()
        .map(|profile| (profile.team, profile.squad))
        .collect();
    assert_eq!(read, kept, "the options say what the server keeps");
    (
        formed.places.iter().map(|place| place.team).collect(),
        options
            .players
            .iter()
            .map(|profile| (profile.team, profile.squad))
            .collect(),
    )
}

/// Every place of a free-for-all, the server's bots too, is on a side of
/// its own (4 + its place) and in no squad, whatever the lobby seated it
/// as; its static info builds, and says the kills that win.
#[test]
fn a_free_for_all_gives_every_place_a_side_of_its_own_and_no_squad() {
    let registry = MatchRegistry::new("127.0.0.1", 1236);
    let ticket = registry.create(seated(5, map_id(), &[3, 4])).unwrap();
    let (teams, options) = sides(&registry, ticket.match_id);
    assert_eq!(teams, [4, 5, 6, 7, 8]);
    assert_eq!(options, [(4, 0), (5, 0), (6, 0), (7, 0), (8, 0)]);
    for (place, &team) in teams.iter().enumerate() {
        assert_eq!(team, maps::free_for_all_team(place));
    }
    let formed = registry.formed(ticket.match_id).unwrap();
    assert_eq!(formed.played.mode(), GameMode::FreeForAll);
    assert_eq!(
        formed.places.iter().map(|place| place.driven).collect::<Vec<_>>(),
        [false, false, false, true, true]
    );
    // srp's entry after the options: tag 3, the kill target.
    assert_eq!(free_for_all().kills_to_win, 30);
    assert!(formed.info.options().ends_with(&srp_entry()));
    // Its players have their seats; a bot has none.
    for place in 0..3 {
        assert_eq!(registry.seat(session_of(place)).map(|seat| seat.place), Some(place as u8));
    }
    assert_eq!(registry.seat(0), None);
}

/// A match of two teams is made as the lobby seated it: its teams and
/// squads stand, and it takes up to twenty.
#[test]
fn a_match_of_two_teams_keeps_the_lobbys_teams_and_squads() {
    let registry = MatchRegistry::new("127.0.0.1", 1236);
    for map_id in [MAP_BATTERIES, MAP_TDM] {
        let ticket = registry.create(seated(5, map_id, &[3, 4])).unwrap();
        let (teams, options) = sides(&registry, ticket.match_id);
        assert_eq!(teams, [0, 1, 0, 1, 0], "map {map_id}");
        assert_eq!(options, [(0, 1), (1, 0), (0, 1), (1, 0), (0, 0)], "map {map_id}");
        let formed = registry.formed(ticket.match_id).unwrap();
        assert!(!formed.info.options().ends_with(&srp_entry()));
        registry.end(ticket.match_id);
    }
}

/// A free-for-all takes as many players as any match, the server's bots
/// counted: the game's 20 places, sides 4 to 23; more is no roster.
#[test]
fn a_free_for_all_takes_as_many_players_as_any_match() {
    let registry = MatchRegistry::new("127.0.0.1", 1236);
    assert_eq!(MAX_PLAYERS, 20);
    // Three players and eighteen of the server's bots.
    let bots: Vec<usize> = (3..MAX_PLAYERS + 1).collect();
    assert_eq!(
        registry.create(seated(MAX_PLAYERS + 1, map_id(), &bots)),
        Err(CreateError::Roster(MAX_PLAYERS + 1))
    );
    // Nothing of a refused match stays.
    assert_eq!(registry.match_count(), 0);
    assert_eq!(registry.seat(session_of(0)), None);
    let ticket = registry.create(seated(MAX_PLAYERS, map_id(), &bots[..17])).unwrap();
    let (teams, _) = sides(&registry, ticket.match_id);
    assert_eq!(teams, (4..24).collect::<Vec<u8>>());
}

/// One taken into a free-for-all being played gets the side of its place,
/// and no squad; a place past the match's 20 is none ([`JoinError::Full`]),
/// one at a time or as a group.
#[test]
fn a_late_joiner_of_a_free_for_all_gets_its_own_side_until_every_place_is_given() {
    let registry = MatchRegistry::new("127.0.0.1", 1236);
    let admit = |match_id: u32, roster: usize| {
        registry.set_admitting(
            match_id,
            roster,
            Ok(Instant::now() + Duration::from_secs(60)),
            LivePhase::Playing,
        );
    };
    let ticket = registry.create_growing(setup(MAX_PLAYERS - 2, map_id())).unwrap();
    let id = ticket.match_id;
    admit(id, MAX_PLAYERS - 2);
    // A group of three has no place: two are left.
    assert_eq!(
        registry.join_running_group(
            id,
            (0..3).map(|n| newcomer(MAX_PLAYERS - 2 + n, 0)).collect()
        ),
        Err(JoinError::Full)
    );
    assert_eq!(registry.formed(id).unwrap().places.len(), MAX_PLAYERS - 2);
    // The lobby put it on side 0, in a squad: its own side, and none.
    assert!(registry.join_running(id, newcomer(MAX_PLAYERS - 2, 2)).is_ok());
    admit(id, MAX_PLAYERS - 1);
    assert!(registry.join_running(id, newcomer(MAX_PLAYERS - 1, 2)).is_ok());
    let (teams, options) = sides(&registry, id);
    assert_eq!(teams, (4..24).collect::<Vec<u8>>());
    assert_eq!(options[MAX_PLAYERS - 2..], [(22, 0), (23, 0)]);
    assert_eq!(
        registry.seat(session_of(MAX_PLAYERS - 1)).map(|seat| seat.place),
        Some(MAX_PLAYERS as u8 - 1)
    );
    // Every place is given: the next is refused, admitting or not.
    assert_eq!(
        registry.join_running(id, newcomer(MAX_PLAYERS, 0)),
        Err(JoinError::Full)
    );
    admit(id, MAX_PLAYERS);
    assert_eq!(
        registry.join_running(id, newcomer(MAX_PLAYERS, 0)),
        Err(JoinError::Full)
    );
    assert_eq!(registry.formed(id).unwrap().places.len(), MAX_PLAYERS);
    assert_eq!(registry.seat(session_of(MAX_PLAYERS)), None);
}

/// A free-for-all played by games: both are given options they read, each
/// place on its own side, and one who comes while it is played enters as
/// the third side, nobody put out of step.
#[test]
fn a_late_joiner_enters_a_free_for_all_being_played() {
    let mut bench = Bench::growing(2, map_id(), Settings::default());
    bench.join_all();
    for client in &bench.clients {
        let options = client.model.options.as_ref().expect("its options");
        assert_eq!(options.map.mode(), GameMode::FreeForAll);
        let teams: Vec<u8> = options.players.iter().map(|player| player.team).collect();
        assert_eq!(teams, [4, 5]);
    }
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert!(bench.game().admitting());
    let place = late_join::reserve(&mut bench);
    bench.join(place);
    assert!(bench.until(8_000, |b| {
        b.clients
            .iter()
            .all(|c| c.entered().contains(&(place as u8)))
    }));
    let formed = bench.registry.formed(bench.match_id).unwrap();
    assert_eq!(
        formed.places.iter().map(|p| p.team).collect::<Vec<_>>(),
        [4, 5, 6]
    );
    let options = bench.clients[place].model.options.as_ref().unwrap();
    assert_eq!(options.players[place].team, 6);
    bench.run_long(5_000);
    for index in 0..=place {
        assert_eq!(bench.game().resyncs(index), 0, "player {index}");
    }
    bench.assert_no_faults();
}

/// A room's free-for-all has no two sides to fill: with two of its four
/// players in (the lobby seated them two a side; each is its own side in
/// the match) it is not called off, and once the start's wait is over the
/// two play.
#[test]
fn a_rooms_free_for_all_starts_with_the_players_who_are_in() {
    let mut bench = Bench::room(&[0, 1, 0, 1], map_id(), Settings::default(), 2);
    bench.join(0);
    bench.join(1);
    // 2 of the 3 needed; places 2 and 3 may still come.
    bench.run_long(25_000);
    assert!(bench.results.try_recv().is_err(), "not called off");
    assert_eq!(bench.game().rule_state(), state::WAITING_FOR_PLAYERS);
    assert!(bench.until(8_000, |b| b.game().rule_state() == state::COUNTDOWN));
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert!(bench.results.try_recv().is_err());
    for place in [0, 1] {
        assert_eq!(bench.game().resyncs(place), 0);
    }
    bench.assert_no_faults();
}

/// A free-for-all in play, for the time `kills_to_win` kills take at the
/// fastest a server believes (one every 2 s).
fn in_play() -> Bench {
    let mut bench = Bench::new(2, map_id(), Settings::default());
    bench.join_all();
    bench.run(4_000);
    bench.run_long(90_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    assert!(bench.results.try_recv().is_err());
    bench
}

/// From the next tick on, player 0 has `kills` kills, all of player 1.
fn kills_from_now(bench: &mut Bench, kills: u16) -> u32 {
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.kills_from = Some((at, kills));
    }
    at
}

/// A player with one kill less than the kills that win ends nothing; with
/// the kill that wins, the match is over the end's delay after the world
/// that shows it. Its result has no team that won and no team points: the
/// winner is read from its players' kills (`session::v026g::rewards`).
#[test]
fn a_free_for_all_ends_when_a_player_has_the_kills_that_win_and_not_one_before() {
    let mut bench = in_play();
    let kills = free_for_all().kills_to_win as u16;
    assert_eq!(kills, 30);
    // A free-for-all's world has no standing of two teams.
    let (_, standing) = bench.game().observed().expect("a trusted snapshot");
    assert_eq!(standing, None);

    kills_from_now(&mut bench, kills - 1);
    bench.run_long(20_000);
    assert!(bench.results.try_recv().is_err(), "one kill short");
    assert!(bench.clients.iter().all(|client| !client.closed));
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);

    let at = kills_from_now(&mut bench, kills);
    assert!(bench.until(12_000, |bench| {
        bench.clients.iter().all(|client| client.closed)
    }));
    let result = bench.results.try_recv().expect("a result");
    assert_eq!(result.end, MatchEnd::ModeRule);
    assert_eq!((result.map_id, result.mode), (map_id(), GameMode::FreeForAll));
    assert_eq!(result.winner, None, "no team wins a free-for-all");
    assert_eq!(result.team_points, [0, 0]);
    assert_eq!(result.trusted_by, 2);
    let ended_after = result.duration_ms - at;
    assert!((3_000..9_500).contains(&ended_after), "{ended_after}");
    assert_eq!((result.players[0].kills, result.players[0].deaths), (kills, 0));
    assert_eq!((result.players[1].kills, result.players[1].deaths), (0, kills));
    assert_eq!(
        result.players.iter().map(|player| player.team).collect::<Vec<_>>(),
        [4, 5]
    );
    assert!(result.players.iter().all(|player| player.present_at_end));
    bench.assert_no_faults();
}

/// What one client claims alone ends no free-for-all.
#[test]
fn the_kills_that_win_claimed_by_one_client_alone_end_nothing() {
    let mut bench = in_play();
    let at = bench.clock() / 50 * 50 + 100;
    bench.clients[0].sim.kills_from = Some((at, free_for_all().kills_to_win as u16));
    bench.run_long(12_000);
    assert!(bench.results.try_recv().is_err());
    assert!(!bench.clients[0].closed && !bench.clients[1].closed);
    bench.assert_no_faults();
}

/// Nobody reaches the kills that win: the match ends at its time limit,
/// with no team that won.
#[test]
fn a_free_for_all_nobody_wins_ends_at_its_time_limit() {
    let mut bench = in_play();
    let limit = u64::from(free_for_all().match_time) * 1_000;
    kills_from_now(&mut bench, 12);
    // The countdown's 20 s and the match's time, less what has passed.
    bench.run_long(limit + 20_000 - 5_000 - bench.ms);
    assert!(bench.results.try_recv().is_err());
    assert!(bench.clients.iter().all(|client| !client.closed));
    assert!(bench.until(30_000, |bench| {
        bench.clients.iter().all(|client| client.closed)
    }));
    let result = bench.results.try_recv().expect("a result");
    assert_eq!(result.end, MatchEnd::TimeLimit);
    assert_eq!(result.mode, GameMode::FreeForAll);
    assert_eq!(result.winner, None);
    let played = u64::from(result.played_ms.expect("play began"));
    assert!((limit..limit + 3_500).contains(&played), "{played}");
    assert_eq!((result.players[0].kills, result.players[1].deaths), (12, 12));
    bench.assert_no_faults();
}


/// A later trusted upload during the closing delay cannot change the
/// winner, counters, statistics, or trusted result tick.
#[test]
fn the_first_terminal_world_owns_the_result_during_the_closing_delay() {
    use survarium::v026g::match_msgs::snapshot::KillsDeaths;
    let mut settings = Settings::default();
    settings.timing.end_delay_ms = 30_000;
    let mut bench = Bench::new(2, map_id(), settings);
    bench.join_all();
    bench.run_long(100_000);
    let at = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.kd_from = Some((at, vec![
            KillsDeaths { kills: 30, deaths: 29, online: true },
            KillsDeaths { kills: 29, deaths: 30, online: true },
        ]));
    }
    assert!(bench.until(10_000, |b| b.game().observed().is_some_and(|(time, _)| time >= at)));
    let terminal = bench.game().observed().unwrap().0;
    let later = bench.clock() / 50 * 50 + 100;
    for client in &mut bench.clients {
        client.sim.kd_from = Some((later, vec![
            KillsDeaths { kills: 30, deaths: 31, online: true },
            KillsDeaths { kills: 31, deaths: 30, online: true },
        ]));
    }
    bench.run_long(12_000);
    assert_eq!(bench.game().observed().unwrap().0, terminal);
    assert!(bench.results.try_recv().is_err());
    assert!(bench.until(25_000, |b| b.clients.iter().all(|c| c.closed)));
    let result = bench.results.try_recv().unwrap();
    assert_eq!(result.end, MatchEnd::ModeRule);
    assert_eq!(result.trusted_snapshot_ms, Some(terminal));
    assert_eq!((result.players[0].kills, result.players[1].kills), (30, 29));
    assert_eq!((result.players[0].deaths, result.players[1].deaths), (29, 30));
    assert!(result.players[0].score > result.players[1].score);
    bench.assert_no_faults();
}

/// A free-for-all with a twist (Racket, `docs/notes/ffa.md` "Twists") arms
/// every profile it gives the clients, its first players' and a late
/// joiner's; the lobby's profiles are as they were, and a plain one's stay
/// as the lobby gave them.
#[test]
fn a_racket_match_arms_every_profile_it_gives_late_joiners_too() {
    use survarium::v026g::twist::{Played, Twist};
    let gun = |name: &str| {
        Dictionary::shared()
            .item_named(name)
            .expect("a gun of the 0.26g dictionary")
            .dict_id
    };
    let sniper = |mut player: MatchPlayer| {
        player.profile.items[0].dict_id = gun("svd");
        player
    };
    let guns = |registry: &MatchRegistry, match_id: u32| -> Vec<u16> {
        let formed = registry.formed(match_id).expect("the match");
        let options = parse_options(formed.info.options(), Dictionary::shared())
            .expect("options a client reads");
        assert_eq!(options.players, formed.profiles, "the clients get what the server keeps");
        formed.profiles.iter().map(|profile| profile.items[0].dict_id).collect()
    };
    let registry = MatchRegistry::new("127.0.0.1", 1237);
    let mut made = setup(2, map_id());
    for player in &mut made.players {
        *player = sniper(player.clone());
    }
    let map = maps::MatchMap::of(map_id()).unwrap();
    made.played = Played::twisted(map, Twist::Racket).expect("a free-for-all's");
    let racket = registry.create_growing(made).unwrap().match_id;
    assert_eq!(registry.formed(racket).unwrap().played.twist(), Some(Twist::Racket));
    assert_eq!(guns(&registry, racket), [gun("ak_12"); 2]);
    registry.set_admitting(
        racket,
        2,
        Ok(Instant::now() + Duration::from_secs(60)),
        LivePhase::Playing,
    );
    assert!(registry.join_running(racket, sniper(newcomer(2, 0))).is_ok());
    assert_eq!(guns(&registry, racket), [gun("ak_12"); 3]);
    // The same players in a plain free-for-all keep their rifles.
    let mut made = setup(2, map_id());
    for player in &mut made.players {
        *player = sniper(player.clone());
        player.session_id += 0x100;
    }
    let plain = registry.create(made).unwrap().match_id;
    assert_eq!(guns(&registry, plain), [gun("svd"); 2]);
}

/// A Cowboy free-for-all (`docs/notes/ffa-cowboy.md`) gives every place's
/// kits the Colt Python alone, in the first slot.
#[test]
fn a_cowboy_match_gives_everyone_a_colt_python() {
    use survarium::v026g::twist::{Played, Twist};
    let registry = MatchRegistry::new("127.0.0.1", 1238);
    let mut made = setup(3, map_id());
    let map = maps::MatchMap::of(map_id()).unwrap();
    made.played = Played::twisted(map, Twist::Cowboy).expect("a free-for-all's");
    let id = registry.create(made).unwrap().match_id;
    let formed = registry.formed(id).unwrap();
    let python = Dictionary::shared().item_named("magnum").unwrap().dict_id;
    for profile in &formed.profiles {
        for kit in &profile.kits {
            let (index, _) = kit.slots[7].expect("a gun in slot 7");
            assert_eq!(profile.items[usize::from(index)].dict_id, python);
            assert_eq!(kit.slots[10], None);
        }
    }
}

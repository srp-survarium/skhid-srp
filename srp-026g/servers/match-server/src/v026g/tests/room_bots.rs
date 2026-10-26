//! A room's bots (its "Bots per team", the owner's of 2026-10-26;
//! `game/driven.rs`): a player who comes into the match takes a bot's place
//! on its team, the bot leaving as a player leaves after the player's entry,
//! a carried battery dropped first; a bot comes back on its own place when
//! a player of its team leaves; in the countdown too, for several joiners
//! at once, and never a bot of a team that can no longer respawn.

use survarium::v026g::match_msgs::rules::state;
use survarium::v026g::match_msgs::snapshot::Tdm;
use survarium::v026g::match_msgs::{ServerMessage, TICK_MS};

use super::*;
use crate::v026g::drive::key;

/// Places 0 and 1 the players (teams 0 and 1), 2 to 5 the room's bots,
/// two a team: 2 and 4 on team 0, 3 and 5 on team 1.
const TEAMS: [u8; 6] = [0, 1, 0, 1, 0, 1];
const BOTS: [usize; 4] = [2, 3, 4, 5];

/// The room's match, both players in and the four bots entered.
fn started(map_id: u8) -> Bench {
    let mut bench = Bench::room_with_bots(&TEAMS, map_id, Settings::default(), 2, &BOTS);
    bench.join(0);
    bench.join(1);
    assert!(
        bench.until(3_000, |bench| BOTS
            .iter()
            .all(|&bot| bench.clients[0].entered().contains(&(bot as u8)))),
        "{:?}",
        bench.clients[0].entered()
    );
    bench
}

/// A new player taken into the match (the lobby's late join) on `team`:
/// its place.
fn reserve_on(bench: &mut Bench, team: u8) -> usize {
    let place = bench.clients.len();
    let player = MatchPlayer {
        session_id: session_of(place),
        account_id: 100 + place as u32,
        profile: profile(&format!("new-player-{place}"), team),
        bot: false,
        driven: false,
    };
    bench
        .registry
        .join_running(bench.match_id, player)
        .expect("the match takes it");
    let map_id = bench.clients[0].sim.map_id;
    bench
        .clients
        .push(Client::growing(session_of(place), place + 1, map_id));
    bench
        .addrs
        .push(SocketAddr::from(([127, 0, 0, 1], 40_000 + place as u16)));
    place
}

/// The roster changes of `place` a client was told, in order: entered
/// (true) or left.
fn changes(client: &Client, place: usize) -> Vec<bool> {
    let mut changes = client.got(|message| match message {
        ServerMessage::PlayerEntered { player, .. } if usize::from(*player) == place => Some(true),
        ServerMessage::PlayerLeft { player, .. } if usize::from(*player) == place => Some(false),
        _ => None,
    });
    changes.dedup();
    changes
}

/// Where in what a client was told the first roster change of `place` to
/// `entered` came.
fn told_at(client: &Client, place: usize, entered: bool) -> Option<usize> {
    client.taken.iter().position(|message| match message {
        ServerMessage::PlayerEntered { player, .. } => entered && usize::from(*player) == place,
        ServerMessage::PlayerLeft { player, .. } => !entered && usize::from(*player) == place,
        _ => false,
    })
}

/// No client was sent a roster change it would drop, nor anything else
/// it chokes on.
fn assert_clean(bench: &Bench) {
    bench.assert_no_faults();
    for (place, client) in bench.clients.iter().enumerate() {
        assert!(
            client.dropped_leaves.is_empty(),
            "player {place}: {:?}",
            client.dropped_leaves
        );
    }
}

/// The owner's of 2026-10-26: "bots, which will be replaced with real
/// players once those join". A player put on team 0 enters, then team 0's
/// last bot (place 4) leaves; before it, the bot pressed drop (a battery it
/// carries goes where it stands) and let every key go. The match has the
/// joiner's place more, none for the bots. When the player leaves, the
/// bot comes back on its own place.
#[test]
fn a_joiner_takes_a_bots_place_on_its_team_and_the_bot_comes_back_when_it_leaves() {
    let mut bench = started(MAP_BATTERIES);
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    let joiner = reserve_on(&mut bench, 0);
    bench.join(joiner);
    assert!(bench.until(8_000, |bench| changes(&bench.clients[0], 4) == [true, false]));
    // Every game hears the player's entry first, then the bot's leave.
    for client in &bench.clients[..2] {
        let entry = told_at(client, joiner, true).expect("the joiner's entry");
        let leave = told_at(client, 4, false).expect("the bot's leave");
        assert!(entry < leave);
    }
    assert_eq!(changes(&bench.clients[joiner], 4), [false]);
    // Team 1's bots and team 0's other one stay.
    for bot in [2, 3, 5] {
        assert_eq!(changes(&bench.clients[0], bot), [true], "bot {bot}");
    }
    // Its last inputs: drop, then nothing pressed.
    let relays = bench.clients[0].relays_of(4);
    let last: Vec<u32> = relays.iter().rev().take(2).map(|input| input.actions).collect();
    assert_eq!(last, [0, key::DROP], "{relays:?}");
    let formed = bench.registry.formed(bench.match_id).unwrap();
    assert_eq!(formed.places.len(), 7);
    // No input of it after its leave.
    bench.run_long(5_000);
    assert_eq!(bench.clients[0].relays_of(4), relays);
    assert!(bench.game().resyncs(0) == 0 && bench.game().resyncs(1) == 0);

    // The player leaves by its own word: team 0 is a player short, and its
    // bot comes back, on its own place.
    bench.clients[joiner].leave();
    assert!(bench.until(8_000, |bench| changes(&bench.clients[0], 4)
        == [true, false, true]));
    assert_eq!(changes(&bench.clients[1], 4), [true, false, true]);
    assert_eq!(bench.registry.formed(bench.match_id).unwrap().places.len(), 7);
    // And it plays again: its script's inputs reach the others.
    let before = bench.clients[0].relays_of(4).len();
    bench.run_long(6_000);
    assert!(bench.clients[0].relays_of(4).len() > before);
    assert_clean(&bench);
}

/// A player who comes in the countdown (the match takes joiners from it
/// on) takes a bot's place there; the countdown runs on to the match.
#[test]
fn a_bot_is_replaced_in_the_countdown() {
    let mut bench = started(MAP_BATTERIES);
    assert_eq!(bench.game().rule_state(), state::COUNTDOWN);
    assert!(bench.until(5_000, |bench| bench.game().admitting()));
    let joiner = reserve_on(&mut bench, 1);
    bench.join(joiner);
    assert!(bench.until(8_000, |bench| changes(&bench.clients[0], 5) == [true, false]));
    assert_eq!(bench.game().rule_state(), state::COUNTDOWN);
    assert!(bench.until(30_000, |bench| bench.game().rule_state() == state::IN_PROCESS));
    assert_eq!(changes(&bench.clients[0], 3), [true]);
    assert_clean(&bench);
}

/// Two players taken in together onto team 1 (a group: they enter on one
/// tick): both of team 1's bots leave, one after the other; team 0's stay.
#[test]
fn several_joiners_at_once_take_as_many_bots_places() {
    let mut bench = started(MAP_BATTERIES);
    bench.run_long(25_000);
    let map_id = bench.clients[0].sim.map_id;
    let group: Vec<MatchPlayer> = [6, 7]
        .into_iter()
        .map(|place| MatchPlayer {
            session_id: session_of(place),
            account_id: 100 + place as u32,
            profile: profile(&format!("new-player-{place}"), 1),
            bot: false,
            driven: false,
        })
        .collect();
    bench
        .registry
        .join_running_group(bench.match_id, group)
        .expect("the match takes them");
    for place in [6, 7] {
        bench
            .clients
            .push(Client::growing(session_of(place), place + 1, map_id));
        bench
            .addrs
            .push(SocketAddr::from(([127, 0, 0, 1], 40_000 + place as u16)));
    }
    bench.clients[6].connect();
    bench.clients[7].connect();
    assert!(bench.until(2_000, |bench| bench.clients[6].model.options.is_some()
        && bench.clients[7].model.options.is_some()));
    bench.clients[6].ready();
    bench.clients[7].ready();
    assert!(bench.until(20_000, |bench| [3, 5]
        .iter()
        .all(|&bot| changes(&bench.clients[0], bot) == [true, false])));
    for bot in [2, 4] {
        assert_eq!(changes(&bench.clients[0], bot), [true], "bot {bot}");
    }
    for joiner in [6, 7] {
        let entry = told_at(&bench.clients[0], joiner, true).expect("in");
        assert!(entry < told_at(&bench.clients[0], 3, false).unwrap());
        assert!(entry < told_at(&bench.clients[0], 5, false).unwrap());
    }
    assert_eq!(bench.registry.formed(bench.match_id).unwrap().places.len(), 8);
    assert_clean(&bench);
}

/// Team Deathmatch, team 1 out of tickets (it can no longer respawn): a
/// player put on it takes no bot's place: a bot may be the team's last
/// life, and the mode's rule ends the match on that.
#[test]
fn no_bot_of_a_team_that_cannot_respawn_gives_way() {
    let mut bench = started(MAP_TDM);
    bench.run_long(25_000);
    assert_eq!(bench.game().rule_state(), state::IN_PROCESS);
    let at = bench.clock() / TICK_MS * TICK_MS + 100;
    for client in &mut bench.clients[..2] {
        client.sim.tdm_from = Some((
            at,
            Tdm {
                tickets: [40, 0],
                alive_players_mask: 0b11_1111,
                active_players_mask: 0b11_1111,
            },
        ));
        client.sim.team_respawn_from = Some((at, [true, false]));
    }
    bench.run_long(8_000);
    let joiner = reserve_on(&mut bench, 1);
    // Its game plays the same world.
    let sim = &bench.clients[0].sim;
    let (tdm, respawn) = (sim.tdm_from, sim.team_respawn_from);
    (
        bench.clients[joiner].sim.tdm_from,
        bench.clients[joiner].sim.team_respawn_from,
    ) = (tdm, respawn);
    bench.join(joiner);
    assert!(bench.until(8_000, |bench| bench.clients[0]
        .entered()
        .contains(&(joiner as u8))));
    bench.run_long(5_000);
    for bot in BOTS {
        assert_eq!(changes(&bench.clients[0], bot), [true], "bot {bot}");
    }
    assert_clean(&bench);
}

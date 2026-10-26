//! A room's bots (its "Bots per team", 0 to 3; the owner's of 2026-10-26:
//! "it should be possible in lobby to add up to 3 vs 3 bots, which will be
//! replaced with real players once those join"): the setting made, kept and
//! told; the match starting with the bots in the places its players leave,
//! a place kept for each bot within the 20; a private room's arranged teams
//! with bots beside the arrangement; a member who comes taking a bot's
//! place, a side full of players and bots too; the maps the bots play told
//! with the pool and a create with another refused; Blackout with bots.

use survarium::v026g::lobby::rooms::{
    BotsPerTeam, POOL_PAGE_ROWS, ROOM_FORM, ROOM_JOIN_BASE, ROOM_PIN_BASE, ROOM_POOL_BASE,
    PoolEntry, RoomSpec, bits_start, create_start,
};
use survarium::v026g::maps;

use super::rooms_tests::{
    arrangement, create, header, join_row, move_member, press_start, rows, spec, state,
};
use super::*;
use crate::v026g::play::Occupant;
use crate::v026g::rooms::BotMaps;
use crate::v026g::{LiveView, MatchSetup};

/// From two players to the match: the vote's 15 s, the wait (15 s), and a
/// little more.
const VOTE_AND_WAIT: Duration = Duration::from_secs(32);
/// The state header's field of the bots a team.
const BOTS: usize = 17;

fn bots(count: u8) -> BotsPerTeam {
    BotsPerTeam::new(count).unwrap()
}

/// A room of `size` a side and `count` bots a team.
fn with_bots(size: u8, count: u8) -> RoomSpec {
    RoomSpec {
        bots: bots(count),
        ..spec(size)
    }
}

/// A folder of the bots' map files (empty: the lobby looks only for the
/// file) of the playable maps `has` takes.
fn bot_map_files(name: &str, has: impl Fn(&maps::MapMode) -> bool) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "srp-room-bot-maps-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for map in maps::playable().filter(|map| has(map)) {
        std::fs::write(dir.join(mapdata::format::file_name(map.project)), b"").unwrap();
    }
    dir
}

/// A lobby whose bots have the walk data of every playable map (they play
/// all but Research).
fn with_bot_maps() -> World {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.rooms.bot_maps = BotMaps::in_dir(bot_map_files("all", |_| true));
    World::with_settings(settings)
}

/// The asker's next room's pool: the entries `take` takes, as the create
/// window sends them (a chunk of 20 entries a request).
fn draft(world: &mut World, index: usize, take: impl Fn(&PoolEntry) -> bool) {
    let entries = world.core.room_entries();
    for (chunk, part) in entries.chunks(20).enumerate() {
        let bits = part
            .iter()
            .enumerate()
            .filter(|(_, entry)| take(entry))
            .fold(0_u32, |bits, (b, _)| bits | 1 << b);
        rows(world, index, bits_start(chunk as u8, bits), 1);
    }
}

/// `names` connected, the first making the room (with bots, of the maps
/// they play, as the window greys the others), the others joining (with
/// the PIN, if it has one): their indices.
fn room_of(world: &mut World, room: RoomSpec, pin: Option<u16>, names: &[&str]) -> (u32, Vec<usize>) {
    let members: Vec<usize> = names.iter().map(|name| world.connect(name)).collect();
    if room.bots != BotsPerTeam::NONE {
        draft(world, members[0], |entry| entry.bots);
    }
    let key = create(world, members[0], room, pin);
    for &member in &members[1..] {
        if let Some(pin) = pin {
            rows(world, member, ROOM_PIN_BASE + u32::from(pin), 1);
        }
        rows(world, member, ROOM_JOIN_BASE + key, 1);
    }
    for &member in &members {
        world.take(member);
    }
    (key, members)
}

/// The match the lobby made last: its setup, and its id in the lobby.
fn last_match(world: &World) -> (u32, MatchSetup, u32) {
    let (ticket, setup) = world.host.made.locked().last().cloned().expect("a match");
    let id = *world
        .core
        .matches
        .iter()
        .find(|(_, running)| running.ticket.match_id == ticket)
        .expect("running")
        .0;
    (ticket, setup, id)
}

/// Of a match's roster: each team's players and server's bots.
fn teams(setup: &MatchSetup) -> [(usize, usize); 2] {
    let on = |team: u8, driven: bool| {
        setup
            .players
            .iter()
            .filter(|player| player.profile.team == team && player.driven == driven)
            .count()
    };
    [(on(0, false), on(0, true)), (on(1, false), on(1, true))]
}

/// The setting is made with the room (the create code's top), told in its
/// state, and kept for its maker's next create window. Its match starts
/// with three bots a team after the two players: the server's own, no
/// account and no session, named as the bots of a match against bots.
#[test]
fn an_open_rooms_match_starts_with_its_bots_in_the_places_its_players_leave() {
    let mut world = with_bot_maps();
    let (_, members) = room_of(&mut world, with_bots(6, 3), None, &["Alice", "Bob"]);
    let alice = members[0];
    assert_eq!(header(&state(&mut world, alice))[BOTS], "3");
    let kept = world.account(alice).room_settings.clone().expect("kept");
    assert_eq!(kept.bots, bots(3));
    let form = rows(&mut world, alice, ROOM_FORM, 1);
    assert!(form[0].ends_with(";3"), "{form:?}");

    world.advance(VOTE_AND_WAIT);
    let (_, setup, id) = last_match(&world);
    assert_eq!(teams(&setup), [(1, 3), (1, 3)]);
    assert_eq!(setup.players.len(), 8);
    // After the players, team 0's first.
    let bot_rows: Vec<(u8, &str)> = setup.players[2..]
        .iter()
        .map(|player| (player.profile.team, player.profile.name.as_str()))
        .collect();
    assert_eq!(
        bot_rows,
        [
            (0, "Bot-1"),
            (0, "Bot-2"),
            (0, "Bot-3"),
            (1, "Bot-4"),
            (1, "Bot-5"),
            (1, "Bot-6")
        ]
    );
    for bot in &setup.players[2..] {
        assert!(bot.driven && !bot.bot);
        assert_eq!((bot.account_id, bot.session_id), (0, 0));
    }
    assert!(!setup.vs_bots && !setup.private);
    assert_eq!(setup.room_minimum, Some(2));
    let running = &world.core.matches[&id];
    assert_eq!(
        running.seats.iter().filter(|(occupant, _)| *occupant == Occupant::Bot).count(),
        6
    );

    // A room of none plays none, as before.
    let mut world = with_bot_maps();
    room_of(&mut world, spec(6), None, &["Carol", "Dave"]);
    world.advance(VOTE_AND_WAIT);
    assert_eq!(teams(&last_match(&world).1), [(1, 0), (1, 0)]);
}

/// The bots fill only places players do not have, and the match keeps a
/// place for each: 7 v 7 in a room of 10 a side and 3 bots a team takes 3
/// bots in all (14 + 2 × 3 = 20), the team with more giving one up first.
#[test]
fn a_rooms_bots_never_take_the_places_its_joiners_need() {
    let mut world = with_bot_maps();
    let names: Vec<String> = (0..14).map(|n| format!("Member{n}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    room_of(&mut world, with_bots(10, 3), None, &names);
    world.advance(VOTE_AND_WAIT);
    let (_, setup, _) = last_match(&world);
    let [(a, a_bots), (b, b_bots)] = teams(&setup);
    assert_eq!((a, b), (7, 7));
    assert_eq!(a_bots + b_bots, 3);
    assert!(a_bots.abs_diff(b_bots) == 1);
    assert_eq!(setup.players.len() + a_bots + b_bots, 20);
    // A room of three a side, 3 bots a team: its sides full of players and
    // bots, 6 + 12 = 18.
    let mut world = with_bot_maps();
    let names = ["A", "B", "C", "D", "E", "F"];
    room_of(&mut world, with_bots(6, 3), None, &names);
    world.advance(VOTE_AND_WAIT);
    assert_eq!(teams(&last_match(&world).1), [(3, 3), (3, 3)]);
}

/// A private room's owner arranges 1 v 3: its match plays that, with two
/// bots a team beside it (the arrangement unchanged: the bots are in no
/// room); it pays nothing, as any private room's.
#[test]
fn a_private_rooms_arranged_teams_get_their_bots_beside_the_arrangement() {
    let mut world = with_bot_maps();
    let (_, members) = room_of(
        &mut world,
        with_bots(6, 2),
        Some(1234),
        &["Alice", "Bob", "Carol", "Dave"],
    );
    let owner = members[0];
    assert_eq!(move_member(&mut world, owner, 2), "0111");
    assert_eq!(header(&state(&mut world, owner))[BOTS], "2");
    assert!(sent_to_match(&press_start(&mut world, owner)).is_some());
    let (_, setup, _) = last_match(&world);
    assert_eq!(teams(&setup), [(1, 2), (3, 2)]);
    assert!(setup.private);
    assert_eq!(setup.room_minimum, Some(1));
    assert_eq!(arrangement(&mut world, owner), "0111");
}

/// The owner's of 2026-10-26: the bots "will be replaced with real players
/// once those join". A room of 6 a side with 3 v 3 players and 3 bots a
/// team plays 6 v 6; a member who comes while it is played is not told the
/// match is full: it goes in, in a bot's place on its side (the match
/// server takes the bot out, `game/driven.rs`), the sides still 6 v 6.
#[test]
fn a_member_who_comes_takes_a_bots_place_on_a_full_side() {
    let mut world = with_bot_maps();
    let names = ["A", "B", "C", "D", "E", "F"];
    let (key, _) = room_of(&mut world, with_bots(6, 3), None, &names);
    world.advance(VOTE_AND_WAIT);
    let (ticket, setup, id) = last_match(&world);
    assert_eq!(teams(&setup), [(3, 3), (3, 3)]);
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present: (1 << 12) - 1,
            points: None,
            kills: [0, 0],
            played: Some(Duration::from_secs(60)),
            time_left: Some(Duration::from_secs(600)),
        },
    );
    world.host.admitting.locked().insert(ticket);
    let carol = world.connect("Newcomer");
    rows(&mut world, carol, ROOM_JOIN_BASE + key, 1);
    let waiting = state(&mut world, carol);
    assert_eq!(join_row(&waiting), Some("J;15"), "{waiting:?}");
    world.take(carol);
    world.advance(Duration::from_secs(16));
    let (sent, team) = sent_to_match(&world.take(carol)).expect("into the match");
    assert_eq!(sent, ticket);
    let running = &world.core.matches[&id];
    assert_eq!(running.seats.len(), 13);
    assert_eq!(running.seats[12].1, team);
    assert_eq!(
        running.seats[12].0.session(),
        Some(world.ids[carol].1)
    );

    // Without the bots the same sides are full: the newcomer is told so.
    let mut world = with_bot_maps();
    let names = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L"];
    let (key, _) = room_of(&mut world, spec(6), None, &names);
    world.advance(VOTE_AND_WAIT);
    let (ticket, setup, _) = last_match(&world);
    assert_eq!(teams(&setup), [(6, 0), (6, 0)]);
    world.host.admitting.locked().insert(ticket);
    let late = world.connect("Late");
    let full = rows(&mut world, late, ROOM_JOIN_BASE + key, 1);
    assert_eq!(header(&full)[1], "N", "a full room takes nobody: {full:?}");
}

/// The owner's of 2026-10-26: "if a player chooses bots, the maps (levels)
/// and modes that bots don't support should be greyed out". The pool's
/// pages say of each entry whether the bots play it: a mode of theirs on a
/// map whose walk data the server has (`SRP_MAPDATA_DIR`), Blackout as its
/// battery map; Research never (`maps::mode::bots_play`: the test data has
/// no Research map in the pool).
#[test]
fn the_pool_says_which_maps_the_bots_play() {
    let first = maps::playable()
        .find(|map| map.mode == maps::mode::BATTERIES)
        .unwrap()
        .level;
    let mut settings = Settings::local("127.0.0.1:0");
    settings.rooms.blackout = true;
    settings.rooms.bot_maps = BotMaps::in_dir(bot_map_files("one", |map| map.level == first));
    let mut world = World::with_settings(settings);
    let alice = world.connect("Alice");
    let entries = world.core.room_entries();
    let mut told = Vec::new();
    for page in 0..entries.len().div_ceil(POOL_PAGE_ROWS) {
        let page_rows = rows(&mut world, alice, ROOM_POOL_BASE + page as u32, 31);
        told.extend(page_rows[1..].iter().map(|row| row.rsplit(';').next().unwrap() == "1"));
    }
    assert_eq!(told.len(), entries.len());
    let mut seen = [false; 2];
    for (entry, &bots) in entries.iter().zip(&told) {
        let played = maps::map_mode(maps::blackout_played(entry.map_id).unwrap_or(entry.map_id));
        let expected = entry.level == first && entry.mode != maps::mode::RESEARCH;
        assert_eq!(bots, expected, "{entry:?} ({played:?})");
        assert_eq!(entry.bots, expected);
        if entry.mode == maps::mode::BLACKOUT && bots {
            seen[0] = true;
        }
        if !bots {
            seen[1] = true;
        }
    }
    assert_eq!(seen, [true, true], "Blackout of theirs, a map without data");
    assert!(!maps::mode::bots_play(maps::mode::RESEARCH));
    assert!(maps::mode::bots_play(maps::BLACKOUT_RULES));
    // No walk data at all: the bots play nothing.
    let mut world = World::new();
    assert!(world.core.room_entries().iter().all(|entry| !entry.bots));
    let bob = world.connect("Bob");
    assert!(rows(&mut world, bob, ROOM_POOL_BASE, 31)[1].ends_with(";0"));
}

/// A room with bots and a map they do not play in its pool is refused
/// (code 8, a bad room: the window greys those maps out); the same pool
/// without bots is made, and so is one of the bots' maps only with them.
#[test]
fn a_room_with_bots_and_a_map_they_do_not_play_is_refused() {
    let first = maps::playable()
        .find(|map| map.mode == maps::mode::BATTERIES)
        .unwrap()
        .level;
    let mut settings = Settings::local("127.0.0.1:0");
    settings.rooms.bot_maps = BotMaps::in_dir(bot_map_files("refused", |map| map.level == first));
    let mut world = World::with_settings(settings);
    let alice = world.connect("Alice");
    let theirs = |entry: &PoolEntry| entry.bots;
    // Theirs and a map with no walk data.
    let elsewhere = |entry: &PoolEntry| entry.bots || entry.level != first;
    draft(&mut world, alice, elsewhere);
    let refused = rows(&mut world, alice, create_start(with_bots(8, 2)), 1);
    assert_eq!(refused[0], "S1;N;8;0;0", "{refused:?}");
    assert!(world.core.rooms.of(world.ids[alice].0).is_none());
    // Without bots: made.
    draft(&mut world, alice, elsewhere);
    let made = rows(&mut world, alice, create_start(spec(8)), 1);
    assert_eq!(header(&made)[1], "R", "{made:?}");
    world.core.rooms.leave(world.ids[alice].0);
    // No draft: the default pool, Research and all: refused with bots.
    let refused = rows(&mut world, alice, create_start(with_bots(8, 1)), 1);
    assert_eq!(refused[0], "S1;N;8;0;0");
    draft(&mut world, alice, theirs);
    let made = rows(&mut world, alice, create_start(with_bots(8, 2)), 1);
    assert_eq!(header(&made)[BOTS], "2");
}

/// The owner's of 2026-10-26: bots play "Protective device: blackout" (the
/// plain battery rules, the interface hidden on the clients). Not greyed
/// out; a room of Blackout alone with bots fills its teams and plays the
/// battery map with the flag.
#[test]
fn a_blackout_room_with_bots_fills_and_plays_the_battery_map() {
    let mut settings = Settings::local("127.0.0.1:0");
    // Rudnya's day: batteries (61), chargers (62), Team Deathmatch (117).
    settings.maps = vec![61, 62, 117];
    settings.rooms.blackout = true;
    settings.rooms.bot_maps = BotMaps::in_dir(bot_map_files("blackout", |_| true));
    let mut world = World::with_settings(settings);
    let blackout = world
        .core
        .room_entries()
        .into_iter()
        .find(|entry| entry.mode == maps::mode::BLACKOUT)
        .expect("Blackout offered");
    assert!(blackout.bots, "not greyed out");
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    draft(&mut world, alice, |entry| entry.mode == maps::mode::BLACKOUT);
    let key = create(&mut world, alice, with_bots(6, 3), None);
    rows(&mut world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(VOTE_AND_WAIT);
    let (_, setup, _) = last_match(&world);
    assert_eq!(setup.map_id, 61, "the battery map");
    assert!(setup.blackout);
    assert_eq!(teams(&setup), [(1, 3), (1, 3)]);
}

/// The review of #577: an open room's max level caps its bots' equipment
/// as it caps its players' (`BotKits::bot_kit_capped`). With no player's
/// kit in the lobby (a new server) the bots wear their roles' fixed kits:
/// a room capped under those plays without bots, one of any level or at
/// their level with them (never over), and a private room has no cap.
#[test]
fn an_open_rooms_bots_never_wear_more_than_its_max_level() {
    // The roles' fixed kits' levels: under the lowest, no bot's kit fits;
    // at the highest, every one does.
    let levels = survarium::v026g::loadouts::BotRole::ALL.map(|role| {
        session::v026g::bot_kits::BotKits::default()
            .bot_kit(0, role, 0)
            .level()
    });
    let (lowest, highest) = (*levels.iter().min().unwrap(), *levels.iter().max().unwrap());
    assert!(lowest >= 1);
    let with_cap = |max_level| RoomSpec {
        max_level,
        ..with_bots(6, 2)
    };
    for (what, room, pin, expected) in [
        ("under the fixed kits", with_cap(lowest - 1), None, [(1, 0), (1, 0)]),
        ("at the fixed kits", with_cap(highest), None, [(1, 2), (1, 2)]),
        ("any", with_cap(0), None, [(1, 2), (1, 2)]),
        ("private", with_cap(lowest - 1), Some(1234), [(1, 2), (1, 2)]),
    ] {
        let mut world = with_bot_maps();
        let (_, members) = room_of(&mut world, room, pin, &["Alice", "Bob"]);
        match pin {
            Some(_) => {
                assert!(sent_to_match(&press_start(&mut world, members[0])).is_some());
            }
            None => world.advance(VOTE_AND_WAIT),
        }
        let (_, setup, _) = last_match(&world);
        // (What each kit is, at or under the cap: `bot_kits` tests.)
        assert_eq!(teams(&setup), expected, "{what}");
    }
}

/// The review of #577: `SRP_PVB_MAX_MATCHES` is the most matches with the
/// server's bots, against bots and rooms' together. At it, a room's match
/// starts without bots (and its players play); Player vs Bots is held to
/// the same count, a room's bot match counted.
#[test]
fn the_bot_matches_limit_counts_rooms_and_starts_the_next_without_bots() {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.rooms.bot_maps = BotMaps::in_dir(bot_map_files("limit", |_| true));
    settings.pvb.max_matches = 1;
    let mut world = World::with_settings(settings);
    // The first room's match takes the one bot match there may be.
    room_of(&mut world, with_bots(6, 2), None, &["Alice", "Bob"]);
    world.advance(VOTE_AND_WAIT);
    let (_, first, _) = last_match(&world);
    assert_eq!(teams(&first), [(1, 2), (1, 2)]);
    assert_eq!(world.core.pvb_running(), 1);
    // The next starts without bots: the players play.
    room_of(&mut world, with_bots(6, 2), None, &["Carol", "Dave"]);
    world.advance(VOTE_AND_WAIT);
    let (ticket, second, _) = last_match(&world);
    assert_eq!(teams(&second), [(1, 0), (1, 0)]);
    assert_eq!(world.core.pvb_running(), 1, "a room's match of no bots counts not");
    // The first's over: a bot match may run again, the next room's has bots.
    let (first_ticket, _) = world.host.made.locked()[0].clone();
    world.end_match(first_ticket, 0);
    world.advance(Duration::from_secs(2));
    assert_eq!(world.core.pvb_running(), 0);
    world.end_match(ticket, 0);
    world.advance(Duration::from_secs(2));
    room_of(&mut world, with_bots(6, 1), None, &["Erin", "Frank"]);
    world.advance(VOTE_AND_WAIT);
    assert_eq!(teams(&last_match(&world).1), [(1, 1), (1, 1)]);
    assert_eq!(world.core.pvb_running(), 1);
}

/// A movie from before the setting makes a room of no bots: its create
/// codes are the first of the codes.
#[test]
fn a_create_from_an_older_movie_makes_a_room_of_no_bots() {
    let mut world = with_bot_maps();
    let alice = world.connect("Alice");
    let room = rows(&mut world, alice, create_start(spec(8)), 1);
    assert_eq!(header(&room)[BOTS], "0");
    assert_eq!(
        create_start(with_bots(8, 1)),
        create_start(spec(8)) + survarium::v026g::lobby::rooms::CREATE_BOTS_STEP
    );
}

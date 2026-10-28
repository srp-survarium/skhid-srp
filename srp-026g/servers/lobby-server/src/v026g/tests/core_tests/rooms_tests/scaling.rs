use super::*;

fn play(world: &mut World, index: usize) {
    rows(world, index, ROOM_PLAY, 1);
}

fn key(world: &World, index: usize) -> u32 {
    world
        .core
        .rooms
        .key_of(world.ids[index].0)
        .expect("in a room")
}

fn full(world: &mut World, label: &str, gear: u8) -> (u32, Vec<usize>) {
    let members: Vec<_> = (0..16)
        .map(|n| world.connect(&format!("{label}{n}")))
        .collect();
    with_gear(world, members[0], gear);
    play(world, members[0]);
    let room = key(world, members[0]);
    for &member in &members[1..] {
        rows(world, member, ROOM_JOIN_BASE + room, 1);
    }
    assert!(world.core.rooms.get(room).unwrap().is_full());
    (room, members)
}

fn waiters(world: &mut World, label: &str, count: usize, gear: u8) -> Vec<usize> {
    (0..count)
        .map(|n| {
            let player = world.connect(&format!("{label}{n}"));
            with_gear(world, player, gear);
            play(world, player);
            player
        })
        .collect()
}

#[test]
fn play_creates_each_bracket_on_demand_and_low_players_can_choose_high_rooms() {
    let mut world = World::new();
    assert!(world.core.rooms.is_empty());
    let high = world.connect("High");
    with_gear(&mut world, high, 15);
    play(&mut world, high);
    let high_key = key(&world, high);
    assert_eq!(world.core.rooms.get(high_key).unwrap().max_level(), 15);
    let low = world.connect("Low");
    with_gear(&mut world, low, 10);
    play(&mut world, low);
    let low_key = key(&world, low);
    assert_ne!(low_key, high_key);
    let room = world.core.rooms.get(low_key).unwrap();
    assert_eq!(
        (room.max_level(), room.spec.size, room.capacity()),
        (10, 8, 16)
    );
    rows(&mut world, low, ROOM_JOIN_BASE + high_key, 1);
    assert_eq!(key(&world, low), high_key);
    assert!(
        world.core.rooms.get(low_key).is_none(),
        "empty rooms disappear"
    );
    assert!(
        !world
            .core
            .rooms
            .get(high_key)
            .unwrap()
            .by_play
            .contains(&world.ids[low].0)
    );
}

#[test]
fn a_client_cannot_create_a_public_room_or_leave_its_room_by_trying() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    assert_eq!(
        rows(&mut world, alice, create_start(spec(8)), 1),
        ["S1;N;16;0;0"]
    );
    assert!(world.core.rooms.is_empty());
    play(&mut world, alice);
    let room = key(&world, alice);
    let refused = rows(&mut world, alice, create_start(spec(8)), 1);
    assert_eq!(header(&refused)[CODE], "16");
    assert_eq!(key(&world, alice), room);
    let private = create(&mut world, alice, spec(8), Some(1234));
    assert!(!world.core.rooms.get(private).unwrap().is_open());
}

#[test]
fn five_wait_but_six_open_another_room_and_all_remain_play_members() {
    let mut world = World::new();
    let (room, _) = full(&mut world, "Member", 1);
    let mut waiting = waiters(&mut world, "Waiting", 5, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 1);
    let position = world.core.rooms.queue_place(world.ids[waiting[0]].0);
    assert_eq!(position, Some((room, 1)));
    play(&mut world, waiting[0]);
    assert_eq!(
        world.core.rooms.queue_place(world.ids[waiting[0]].0),
        position
    );
    assert_eq!(world.core.rooms.queued_total(), 5);
    waiting.extend(waiters(&mut world, "Sixth", 1, 1));
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 2);
    let next = key(&world, waiting[0]);
    assert_ne!(next, room);
    for &player in &waiting {
        assert_eq!(key(&world, player), next);
        assert!(
            world
                .core
                .rooms
                .get(next)
                .unwrap()
                .by_play
                .contains(&world.ids[player].0)
        );
    }
    assert_eq!(world.core.rooms.queued_total(), 0);
    // Their Play still means one match, including after scaling.
    let accounts: Vec<_> = waiting.iter().map(|&index| world.ids[index].0).collect();
    world
        .core
        .rooms
        .started(next, 123, 61, &accounts, world.now);
    world.core.room_match_over(123);
    assert!(world.core.rooms.get(next).is_none());
}

#[test]
fn a_freed_place_admits_a_play_waiter_without_opening_another_room() {
    let mut world = World::new();
    let (room, members) = full(&mut world, "Member", 1);
    let waiting = waiters(&mut world, "Waiting", 5, 1);
    rows(&mut world, members[1], ROOM_LEAVE, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 1);
    assert_eq!(key(&world, waiting[0]), room);
    assert!(
        world
            .core
            .rooms
            .get(room)
            .unwrap()
            .by_play
            .contains(&world.ids[waiting[0]].0)
    );
    assert_eq!(world.core.rooms.queued_total(), 4);
}

#[test]
fn explicitly_chosen_queues_do_not_expand_or_move_with_play_waiters() {
    let mut world = World::new();
    let (room, _) = full(&mut world, "Member", 1);
    let chosen: Vec<_> = (0..6)
        .map(|n| {
            let player = world.connect(&format!("Chosen{n}"));
            rows(&mut world, player, ROOM_QUEUE_BASE + room, 1);
            player
        })
        .collect();
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 1);
    let waiting = waiters(&mut world, "Waiting", 6, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 2);
    assert_ne!(key(&world, waiting[0]), room);
    for (i, player) in chosen.into_iter().enumerate() {
        assert_eq!(
            world.core.rooms.queue_place(world.ids[player].0),
            Some((room, i + 1))
        );
    }
}

#[test]
fn leaving_the_queue_and_unplayable_loadouts_do_not_count_toward_six() {
    let mut world = World::new();
    full(&mut world, "Member", 1);
    let waiting = waiters(&mut world, "Waiting", 5, 1);
    rows(&mut world, waiting[0], ROOM_UNQUEUE, 1);
    waiters(&mut world, "Fifth", 1, 1);
    assert_eq!(world.core.rooms.len(), 1);
    let slots = world.account(waiting[1]).kits[0].slots;
    world.change(waiting[1], |account| account.kits[0].slots = [None; 23]);
    waiters(&mut world, "Sixth", 1, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 1, "only five can play");
    world.change(waiting[1], |account| account.kits[0].slots = slots);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 2);
}

#[test]
fn changed_equipment_routes_a_play_waiter_to_its_new_bracket() {
    let mut world = World::new();
    let (low, _) = full(&mut world, "Member", 1);
    let waiting = waiters(&mut world, "Waiting", 1, 10)[0];
    with_gear(&mut world, waiting, 15);
    world.advance(Duration::from_secs(1));
    let high = key(&world, waiting);
    assert_ne!(high, low);
    assert_eq!(world.core.rooms.get(high).unwrap().max_level(), 15);
    assert_eq!(world.core.rooms.queued_total(), 0);
}

#[test]
fn failed_expansion_keeps_every_waiter_and_retries_when_capacity_is_available() {
    let mut world = World::new();
    world.core.settings.rooms.max_rooms = 1;
    let (room, _) = full(&mut world, "Member", 1);
    let waiting = waiters(&mut world, "Waiting", 6, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 1);
    for &player in &waiting {
        assert_eq!(
            world.core.rooms.queue_place(world.ids[player].0).unwrap().0,
            room
        );
    }
    world.core.settings.rooms.max_rooms = 2;
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 2);
    assert_eq!(world.core.rooms.queued_total(), 0);
}

#[test]
fn six_waiters_across_two_full_rooms_expand_together() {
    let mut world = World::new();
    full(&mut world, "Member", 1);
    let second = waiters(&mut world, "Second", 6, 1);
    world.advance(Duration::from_secs(1));
    let second_key = key(&world, second[0]);
    waiters(&mut world, "Filling", 10, 1);
    assert!(world.core.rooms.get(second_key).unwrap().is_full());
    let mut waiting = waiters(&mut world, "Waiting", 5, 1);
    let keys: std::collections::HashSet<_> = waiting
        .iter()
        .map(|&player| world.core.rooms.queue_place(world.ids[player].0).unwrap().0)
        .collect();
    assert_eq!(keys.len(), 2, "demand is split across both queues");
    waiting.extend(waiters(&mut world, "Sixth", 1, 1));
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 3);
    let next = key(&world, waiting[0]);
    assert!(!keys.contains(&next));
    assert!(waiting.iter().all(|&player| key(&world, player) == next));
}

#[test]
fn a_room_that_empties_after_its_match_admits_its_waiters() {
    let mut world = World::new();
    let (room, members) = full(&mut world, "Member", 1);
    let waiting = waiters(&mut world, "Waiting", 5, 1);
    let accounts: Vec<_> = members.iter().map(|&player| world.ids[player].0).collect();
    for &account in &accounts {
        world.core.rooms.came_by_play(account);
    }
    world
        .core
        .rooms
        .started(room, 123, 61, &accounts, world.now);
    world.core.room_match_over(123);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 1);
    assert!(waiting.iter().all(|&player| key(&world, player) == room));
    assert_eq!(world.core.rooms.queued_total(), 0);
    assert_eq!(
        world.core.rooms.get(room).unwrap().owner,
        world.ids[waiting[0]].0
    );
}

#[test]
fn a_restarted_game_does_not_count_toward_expansion_until_it_knows_its_queue() {
    let mut world = World::new();
    full(&mut world, "Member", 1);
    let waiting = waiters(&mut world, "Waiting", 5, 1);
    let account = world.ids[waiting[0]].0;
    let session = world.new_session(account);
    let again = world.connect_session((account, session));
    waiters(&mut world, "Sixth", 1, 1);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 1);
    state(&mut world, again);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 2);
}

#[test]
fn a_new_play_does_not_jump_a_queue_when_a_place_has_just_freed() {
    let mut world = World::new();
    let (room, members) = full(&mut world, "Member", 1);
    let waiting = waiters(&mut world, "Waiting", 1, 1)[0];
    let newcomer = world.connect("Newcomer");
    // A Leave and a Play arrive before the next admission pass.
    world.core.rooms.leave(world.ids[members[1]].0);
    world.core.handle(
        world.clients[newcomer].0,
        ClientMessage::QueryRanking {
            start: ROOM_PLAY,
            count: 1,
        },
        world.now,
    );
    assert!(world.core.rooms.of(world.ids[newcomer].0).is_none());
    world.advance(Duration::from_secs(1));
    assert_eq!(key(&world, waiting), room);
    assert_eq!(
        world.core.rooms.queue_place(world.ids[newcomer].0),
        Some((room, 1))
    );
}

#[test]
fn three_waiters_in_each_bracket_do_not_open_another_room() {
    let mut world = World::new();
    full(&mut world, "Low", 1);
    full(&mut world, "High", 15);
    waiters(&mut world, "LowWait", 3, 1);
    waiters(&mut world, "HighWait", 3, 15);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.core.rooms.len(), 2);
    assert_eq!(world.core.rooms.queued_total(), 6);
}

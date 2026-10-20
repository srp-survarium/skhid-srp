//! "Show player's information" outside the results screen (`gear`): the
//! kits of any account, as it is now, with the results screen's `93`.

use survarium::v026g::lobby::GEAR_BY_PLACE;

use super::*;

/// The name and the kit count of each `93` the client took.
fn gear(world: &mut World, index: usize) -> Vec<(String, usize)> {
    world
        .take(index)
        .into_iter()
        .filter_map(|message| match message {
            Decoded::PlayerDetails { player, .. } => Some((player.name, player.kits.len())),
            _ => None,
        })
        .collect()
}

/// `count` accounts that never signed in since the start (as srp.db gives
/// them: offline, no match ever), best rated first: `Ranked0` above all.
fn ranked_accounts(world: &World, count: u32) -> Vec<String> {
    (0..count)
        .map(|i| {
            let name = format!("Ranked{i}");
            let (account, _) = world.accounts.sign_up(900_000 + i, &name);
            account.locked().rating = 5000 - i as u16;
            name
        })
        .collect()
}

/// A right-click on any row of the Rankings window (the owner's of
/// 2026-10-20: "for every single player"): the top of a long ranking, its
/// bottom, the asker's own row, offline accounts that never played a
/// match, each with its kits as they are now.
#[test]
fn every_row_of_the_ranking_shows_its_players_kits() {
    let mut world = World::new();
    let names = ranked_accounts(&world, 300);
    let alice = world.connect("Alice");
    world.account(alice).rating = 1;
    // The top 100, as the page opens.
    world.ask(alice, request::ranking(0, 100));
    world.take(alice);
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 1));
    let top = gear(&mut world, alice);
    let kits = world.accounts.get(900_000).unwrap().locked().kits.len();
    assert!(kits > 0);
    assert_eq!(top, [(names[0].clone(), kits)]);
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 100));
    assert_eq!(gear(&mut world, alice), [(names[99].clone(), kits)]);
    // "My place": the bottom of the ranking, Alice's own row last.
    world.ask(alice, request::ranking(201, 100));
    world.take(alice);
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 300));
    assert_eq!(gear(&mut world, alice), [(names[299].clone(), kits)]);
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 301));
    assert_eq!(gear(&mut world, alice), [("Alice".to_owned(), kits)]);
    // A place no page of hers was sent: the ranking's now.
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 150));
    assert_eq!(gear(&mut world, alice), [(names[149].clone(), kits)]);
    // No such place: no answer (no window opens).
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 0));
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 302));
    world.ask(alice, request::player_details(GEAR_BY_PLACE, u32::MAX));
    assert!(gear(&mut world, alice).is_empty());
    world.assert_clean();
}

/// The row clicked is the player the page showed there, though the ranking
/// moved since; its kits are those it has now (not a match's).
#[test]
fn a_row_is_the_player_the_page_showed_with_its_kits_now() {
    let mut world = World::new();
    let names = ranked_accounts(&world, 3);
    let alice = world.connect("Alice");
    world.ask(alice, request::ranking(0, 10));
    world.take(alice);
    // Ranked2 goes to the top after the page was drawn, and loses a kit.
    {
        let account = world.accounts.get(900_002).unwrap();
        let mut account = account.locked();
        account.rating = 9000;
        account.kits.pop();
    }
    let kits = world.accounts.get(900_002).unwrap().locked().kits.len();
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 3));
    assert_eq!(gear(&mut world, alice), [(names[2].clone(), kits)]);
    // The page opened again: the ranking as it is now.
    world.ask(alice, request::ranking(0, 10));
    world.take(alice);
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 1));
    assert_eq!(gear(&mut world, alice), [(names[2].clone(), kits)]);
    world.assert_clean();
}

/// The own place under the list is the asker, though another player has
/// taken that place since; a cached tab's rows are what that tab showed,
/// though a newer page of the other tab was sent since.
#[test]
fn the_own_place_and_a_cached_tab_are_what_they_showed() {
    let mut world = World::new();
    let names = ranked_accounts(&world, 300);
    let alice = world.connect("Alice");
    world.account(alice).rating = 1;
    // Top 100 (Alice is 301st, under the list), then the "My place" tab.
    world.ask(alice, request::ranking(0, 100));
    world.take(alice);
    world.ask(alice, request::ranking(201, 100));
    world.take(alice);
    // Alice goes to the top and Ranked0 to the bottom.
    world.account(alice).rating = 9000;
    world.accounts.get(900_000).unwrap().locked().rating = 0;
    let kits = world.accounts.get(900_000).unwrap().locked().kits.len();
    // The own place under the list (301) is Alice, not Ranked0 now there.
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 301));
    assert_eq!(gear(&mut world, alice), [("Alice".to_owned(), kits)]);
    // The cached Top 100 tab: place 1 is Ranked0, as it showed.
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 1));
    assert_eq!(gear(&mut world, alice), [(names[0].clone(), kits)]);
    // The "My place" tab, sent last: place 250 as it showed.
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 250));
    assert_eq!(gear(&mut world, alice), [(names[249].clone(), kits)]);
    world.assert_clean();
}

/// A session keeps its last pages only: a place of a page long gone is the
/// ranking's now.
#[test]
fn only_the_last_pages_are_kept() {
    let mut world = World::new();
    let names = ranked_accounts(&world, 300);
    let alice = world.connect("Alice");
    world.ask(alice, request::ranking(0, 10));
    world.take(alice);
    for page in 1..=10 {
        world.ask(alice, request::ranking(page * 20, 10));
        world.take(alice);
    }
    // Ranked0 drops to the bottom: place 1 is Ranked1 now.
    world.accounts.get(900_000).unwrap().locked().rating = 0;
    let kits = world.accounts.get(900_001).unwrap().locked().kits.len();
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 1));
    assert_eq!(gear(&mut world, alice), [(names[1].clone(), kits)]);
    // A page still kept: what it showed.
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 201));
    assert_eq!(gear(&mut world, alice), [(names[200].clone(), kits)]);
    world.assert_clean();
}

//! Hero of the day results / titles and the bought specifics (schema v13).

use accounts::{HashCost, HeroResult, HeroTitle, SpecificError, Store};

fn mem() -> Store {
    let s = Store::open_in_memory().unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    s
}

#[test]
fn hero_results_slots_and_titles() {
    let s = mem();
    let a = s.create_account("alpha", "pw").unwrap();
    let b = s.create_account("beta", "pw").unwrap();
    let r = |account, at| HeroResult { account, at, group: 0, score: 10, killstreak: 1, won: true };
    s.hero_add_result(&r(a, 100)).unwrap();
    s.hero_add_result(&r(b, 100)).unwrap();
    s.hero_add_result(&r(a, 200)).unwrap();
    // `(from, to]`, oldest first, ties by insertion
    let rows = s.hero_results_between(100, 200).unwrap();
    assert_eq!(rows.iter().map(|x| (x.account, x.at)).collect::<Vec<_>>(), vec![(a, 200)]);
    assert_eq!(s.hero_results_between(0, 200).unwrap().iter().map(|x| x.account).collect::<Vec<_>>(), vec![a, b, a]);
    assert_eq!(s.hero_slot().unwrap(), None);
    let t = |account, place, since, until| HeroTitle { account, group: 0, nomination: 1, place, value: 10, since, until };
    s.hero_commit_slot(1000, &[t(a, 1, 1000, 2000), t(b, 2, 1000, 2000)], 150).unwrap();
    assert_eq!(s.hero_slot().unwrap(), Some(1000));
    // results at <= 150 were purged
    assert_eq!(s.hero_results_between(0, 1000).unwrap().len(), 1);
    // the ranking is the newest slot that still lives; a title of an account is also in its profile
    assert_eq!(s.hero_current_ranking(1500).unwrap().iter().map(|x| (x.account, x.place)).collect::<Vec<_>>(), vec![(a, 1), (b, 2)]);
    assert_eq!(s.load_profile(a).unwrap().hero, vec![t(a, 1, 1000, 2000)]);
    assert!(s.hero_current_ranking(2000).unwrap().is_empty());
    assert!(s.hero_titles_of(a, 2000).unwrap().is_empty());
    // a newer slot replaces the ranking; titles that ended before it are deleted by the commit
    s.hero_commit_slot(2500, &[t(b, 1, 2500, 3500)], 0).unwrap();
    assert_eq!(s.hero_current_ranking(2600).unwrap().iter().map(|x| x.account).collect::<Vec<_>>(), vec![b]);
    assert!(s.load_profile(a).unwrap().hero.is_empty());
    // saving a profile never writes the derived titles
    let mut p = s.load_profile(b).unwrap();
    p.silver += 1;
    s.save_profile(b, &p).unwrap();
    assert_eq!(s.load_profile(b).unwrap().hero.len(), 1);
}

#[test]
fn buying_a_specific_is_one_transaction() {
    let s = mem();
    let a = s.create_account("alpha", "pw").unwrap();
    let gold = s.load_profile(a).unwrap().gold;
    assert_eq!(s.buy_specific(a, 736, gold + 1, 5), Err(SpecificError::NotEnoughGold));
    assert!(s.specifics_opened(a).unwrap().is_empty());
    assert_eq!(s.load_profile(a).unwrap().gold, gold, "a refused buy spends nothing");
    s.buy_specific(a, 736, 200, 5).unwrap();
    assert_eq!(s.load_profile(a).unwrap().gold, gold - 200);
    assert_eq!(s.specifics_opened(a).unwrap(), vec![736]);
    assert_eq!(s.buy_specific(a, 736, 200, 6), Err(SpecificError::AlreadyOpened));
    assert_eq!(s.load_profile(a).unwrap().gold, gold - 200);
    // another account is unaffected
    let b = s.create_account("beta", "pw").unwrap();
    assert!(s.specifics_opened(b).unwrap().is_empty());
}

#[test]
fn a_database_at_v12_is_migrated_to_v13() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.sqlite");
    {
        let s = Store::open(&path).unwrap();
        s.set_hash_cost(HashCost::FAST_FOR_TESTS);
        s.create_account("alpha", "pw").unwrap();
    }
    // pretend the file was last written by the v12 server: the v13 tables are gone and the version is 12
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("DROP TABLE hero_results; DROP TABLE hero_titles; DROP TABLE hero_meta; DROP TABLE specifics_opened; PRAGMA user_version = 12;").unwrap();
    }
    let s = Store::open(&path).unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 23);
    let id = s.find_account("alpha").unwrap().unwrap().id;
    s.buy_specific(id, 5, 1, 1).unwrap();
    assert_eq!(s.hero_slot().unwrap(), None);
    // opening again is harmless (the statements are idempotent)
    drop(s);
    Store::open(&path).unwrap();
}

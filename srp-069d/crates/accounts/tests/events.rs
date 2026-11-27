//! The event currency wallet (schema v17, `event_wallet`): saved with the profile, spent and credited like the other
//! wallets, and added by an idempotent migration.

use accounts::{Currency, HashCost, Store};

#[test]
fn the_event_wallet_is_saved_with_the_profile() {
    let s = Store::open_in_memory().unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    let a = s.create_account("alpha", "pw").unwrap();
    assert_eq!(s.load_profile(a).unwrap().event, [0, 0, 0]);
    s.update_profile(a, |p| {
        p.credit(Currency::Event1, 10);
        p.credit(Currency::Event3, 7);
        Ok::<_, ()>(())
    })
    .unwrap();
    let p = s.load_profile(a).unwrap();
    assert_eq!(p.event, [10, 0, 7]);
    assert_eq!((p.balance(Currency::Event1), p.balance(Currency::Event2), p.balance(Currency::Event3)), (10, 0, 7));
    // spending more than held changes nothing
    s.update_profile(a, |p| Ok::<_, ()>(assert!(p.spend(Currency::Event3, 8).is_err()))).unwrap();
    assert_eq!(s.load_profile(a).unwrap().event, [10, 0, 7]);
    s.update_profile(a, |p| Ok::<_, ()>(p.spend(Currency::Event1, 10).unwrap())).unwrap();
    assert_eq!(s.load_profile(a).unwrap().event, [0, 0, 7]);
    // the wallet is per account
    let b = s.create_account("beta", "pw").unwrap();
    assert_eq!(s.load_profile(b).unwrap().event, [0, 0, 0]);
}

#[test]
fn a_database_at_v16_gets_the_event_wallet_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.sqlite");
    let a;
    {
        let s = Store::open(&path).unwrap();
        s.set_hash_cost(HashCost::FAST_FOR_TESTS);
        a = s.create_account("alpha", "pw").unwrap();
        s.update_profile(a, |p| Ok::<_, ()>(p.credit(Currency::Event2, 5))).unwrap();
    }
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("DROP TABLE event_wallet; PRAGMA user_version = 16;").unwrap();
    }
    let s = Store::open(&path).unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 23);
    // the dropped balance is gone (it was a v17 table), the wallet works again
    assert_eq!(s.load_profile(a).unwrap().event, [0, 0, 0]);
    s.update_profile(a, |p| Ok::<_, ()>(p.credit(Currency::Event2, 9))).unwrap();
    assert_eq!(s.load_profile(a).unwrap().event, [0, 9, 0]);
    drop(s);
    // running the migration again changes nothing
    let s = Store::open(&path).unwrap();
    assert_eq!(s.load_profile(a).unwrap().event, [0, 9, 0]);
}

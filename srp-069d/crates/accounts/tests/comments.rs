//! The complaints / compliments table is added by the v16 migration (idempotent).

use accounts::{HashCost, Store};

#[test]
fn a_database_at_v15_gets_the_comments_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("m.sqlite");
    let (a, b);
    {
        let s = Store::open(&path).unwrap();
        s.set_hash_cost(HashCost::FAST_FOR_TESTS);
        a = s.create_account("alpha", "pw").unwrap();
        b = s.create_account("beta", "pw").unwrap();
    }
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("DROP TABLE user_comments; PRAGMA user_version = 15;").unwrap();
    }
    let s = Store::open(&path).unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    let v: i64 = rusqlite::Connection::open(&path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 23);
    assert_eq!(s.add_comment(a, b, 1, 0, 0, None).unwrap(), accounts::comments::CommentResult::Added);
    assert_eq!(s.list_comments(Some(b), 5).unwrap().len(), 1);
    drop(s);
    Store::open(&path).unwrap();
}

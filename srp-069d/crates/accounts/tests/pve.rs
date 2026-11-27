//! PvE storage (schema v13): migration from v12, results and ladder, chests, daily wins.

use accounts::{HashCost, PveRewardRow, PveRun, PveRunPlayer, Store};

fn mem() -> Store {
    let s = Store::open_in_memory().unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    s
}

fn file_store(path: &std::path::Path) -> Store {
    let s = Store::open(path).unwrap();
    s.set_hash_cost(HashCost::FAST_FOR_TESTS);
    s
}

fn user_version(path: &std::path::Path) -> i64 {
    rusqlite::Connection::open(path).unwrap().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
}

/// Schema v13 (PvE): a fresh database is at 13, a v12 file migrates (idempotent tables) and keeps its data.
#[test]
fn pve_tables_are_v13_and_a_v12_file_migrates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v13.sqlite");
    let id = {
        let s = file_store(&path);
        s.create_account("amy", "pw").unwrap()
    };
    assert_eq!(user_version(&path), 23);
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("DROP TABLE pve_daily_wins; DROP TABLE pve_rewards; DROP TABLE pve_result_players; DROP TABLE pve_results; PRAGMA user_version = 13;").unwrap();
    }
    let s = file_store(&path);
    assert_eq!(user_version(&path), 23);
    assert_eq!(s.account(id).unwrap().unwrap().name, "amy");
    assert_eq!(s.pve_wins_on(id, 1).unwrap(), 0);
    drop(s);
    // opening again changes nothing
    drop(file_store(&path));
    assert_eq!(user_version(&path), 23);
}

fn run(match_id: u32, players: &[(i64, u16)], success: bool, chests: u8, time: u32) -> PveRun {
    PveRun {
        match_id,
        mission: 15,
        difficulty: 1,
        success,
        time_secs: time,
        chests_found: chests,
        chests_total: 10,
        started_at: 1000,
        players: players
            .iter()
            .enumerate()
            .map(|(i, (a, score))| PveRunPlayer { account: *a, score: *score, kills: 1, deaths: 0, sort: i as u8, premium: false })
            .collect(),
    }
}

#[test]
fn pve_results_ladder_rewards_and_daily_wins() {
    let s = mem();
    let (a, b, c) = (s.create_account("amy", "pw").unwrap(), s.create_account("ben", "pw").unwrap(), s.create_account("cat", "pw").unwrap());
    // the first run of a wire id is stored under the wire id itself
    assert_eq!(s.pve_add_result(&run(1, &[(a, 10), (b, 20)], true, 5, 900)).unwrap(), 1);
    // the same squad again (other order of ids): more chests but slower, then the same chests and faster
    assert_eq!(s.pve_add_result(&run(2, &[(b, 1), (a, 2)], true, 7, 1200)).unwrap(), 2);
    assert_eq!(s.pve_add_result(&run(3, &[(a, 1), (b, 2)], true, 7, 1000)).unwrap(), 3);
    // a failed run and a solo run of cat, a solo run of amy
    assert_eq!(s.pve_add_result(&run(4, &[(c, 1)], false, 10, 10)).unwrap(), 4);
    assert_eq!(s.pve_add_result(&run(5, &[(c, 5)], true, 3, 500)).unwrap(), 5);
    assert_eq!(s.pve_add_result(&run(6, &[(a, 5)], true, 3, 400)).unwrap(), 6);
    assert!(s.pve_add_result(&run(7, &[], true, 3, 400)).is_err());

    let l = s.pve_ladder(15, 100).unwrap();
    // best run per squad, best first: (a,b) 7 chests 1000 s, then a alone 3 chests 400 s, then c 3 chests 500 s
    assert_eq!(l.iter().map(|r| r.match_id).collect::<Vec<_>>(), vec![3, 6, 5]);
    assert_eq!(l.iter().map(|r| r.run_key).collect::<Vec<_>>(), vec![3, 6, 5]);
    assert_eq!(s.pve_ladder(15, 2).unwrap().len(), 2);
    assert!(s.pve_ladder(14, 100).unwrap().is_empty());
    let p = s.pve_run_players(l[0].result_id).unwrap();
    assert_eq!(p.iter().map(|x| (x.account, x.score)).collect::<Vec<_>>(), vec![(a, 1), (b, 2)]);
    let (stored, _) = s.pve_result(3).unwrap().unwrap();
    assert_eq!((stored.chests_found, stored.time_secs, stored.success), (7, 1000, true));
    assert!(s.pve_result(99).unwrap().is_none());

    // chests: idempotent insert, update
    let row = PveRewardRow { idx: 0, chest_dict: 1385, tier: 2, difficulty: 1, opened: false, reward_dict: 0, reward_amount: 0, rerolls: 0, paid: false };
    s.pve_add_rewards(a, 3, &[row, PveRewardRow { idx: 1, ..row }]).unwrap();
    let opened = PveRewardRow { opened: true, reward_dict: 66, reward_amount: 3, rerolls: 1, paid: true, ..row };
    assert!(s.pve_update_reward(a, 3, &opened).unwrap());
    s.pve_add_rewards(a, 3, &[row]).unwrap();
    assert_eq!(s.pve_rewards(a, 3).unwrap(), vec![opened, PveRewardRow { idx: 1, ..row }]);
    assert!(s.pve_rewards(b, 3).unwrap().is_empty());
    assert!(!s.pve_update_reward(b, 3, &opened).unwrap());

    // wins of a day
    assert_eq!(s.pve_note_win(a, 20000).unwrap(), 1);
    assert_eq!(s.pve_note_win(a, 20000).unwrap(), 2);
    assert_eq!(s.pve_wins_on(a, 20000).unwrap(), 2);
    assert_eq!(s.pve_wins_on(a, 20001).unwrap(), 0);
}

/// A PvE match id is the captured world's id, so every run of one maze has the same wire id: each
/// run is stored under its own key (wire id | n << 32), with its own chests; the latest run of a player is the
/// new one; the ladder shows the wire id. A failure stored after a success of the same wire id is stored too.
#[test]
fn runs_with_the_same_wire_id_get_their_own_keys_and_chests() {
    use accounts::{pve_run_key, pve_wire_id};
    let s = mem();
    let (a, b) = (s.create_account("amy", "pw").unwrap(), s.create_account("ben", "pw").unwrap());
    let wire = 0x1002_4cad;
    let k0 = s.pve_add_result(&run(wire, &[(a, 10)], true, 5, 900)).unwrap();
    assert_eq!(k0, i64::from(wire));
    let k1 = s.pve_add_result(&run(wire, &[(a, 3)], false, 0, 300)).unwrap();
    assert_eq!(k1, pve_run_key(wire, 1));
    assert_eq!(k1, 0x1_1002_4cad);
    let k2 = s.pve_add_result(&run(wire, &[(b, 7), (a, 1)], true, 9, 600)).unwrap();
    assert_eq!(k2, pve_run_key(wire, 2));
    assert_eq!((pve_wire_id(k0), pve_wire_id(k1), pve_wire_id(k2)), (wire, wire, wire));
    // a wire id with the top bit set (u32 > i32::MAX) keeps its low bits
    let big = 0xF000_0001;
    assert_eq!(s.pve_add_result(&run(big, &[(b, 1)], true, 1, 1)).unwrap(), 0xF000_0001);
    assert_eq!(s.pve_add_result(&run(big, &[(b, 1)], true, 1, 1)).unwrap(), 0x1_F000_0001);
    // every run reads back with the wire id and its own numbers
    let (r1, _) = s.pve_result(k1).unwrap().unwrap();
    assert_eq!((r1.match_id, r1.success, r1.time_secs), (wire, false, 300));
    let (r2, _) = s.pve_result(k2).unwrap().unwrap();
    assert_eq!((r2.match_id, r2.chests_found, r2.players.len()), (wire, 9, 2));
    // the latest run of each player is the newest one he took part in
    assert_eq!(s.pve_latest_run(a).unwrap(), Some(k2));
    assert_eq!(s.pve_latest_run(b).unwrap(), Some(pve_run_key(big, 1)));
    // chests of two runs with one wire id stay apart
    let row = PveRewardRow { idx: 0, chest_dict: 1385, tier: 2, difficulty: 1, opened: false, reward_dict: 0, reward_amount: 0, rerolls: 0, paid: false };
    s.pve_add_rewards(a, k0, &[row]).unwrap();
    s.pve_add_rewards(a, k2, &[row, PveRewardRow { idx: 1, ..row }]).unwrap();
    assert_eq!(s.pve_rewards(a, k0).unwrap().len(), 1);
    assert_eq!(s.pve_rewards(a, k2).unwrap().len(), 2);
    assert!(s.pve_rewards(a, k1).unwrap().is_empty());
    // the ladder: the squad 9 chests (k2), amy alone 5 chests (k0), ben alone (his older equal run); all show the wire id
    let l = s.pve_ladder(15, 100).unwrap();
    assert_eq!(l.iter().map(|r| (r.run_key, r.match_id)).collect::<Vec<_>>(), vec![(k2, wire), (k0, wire), (0xF000_0001, big)]);
}

/// The end screen's chest change: rows and profile commit together, only on the expected rows; a refusal writes nothing.
#[test]
fn chest_rows_and_profile_change_in_one_transaction() {
    use accounts::{ChestRefusal, UpdateError};
    let s = mem();
    let a = s.create_account("amy", "pw").unwrap();
    let row = PveRewardRow { idx: 0, chest_dict: 1483, tier: 1, difficulty: 0, opened: false, reward_dict: 0, reward_amount: 0, rerolls: 0, paid: false };
    let opened = PveRewardRow { opened: true, reward_dict: 66, reward_amount: 3, ..row };
    let silver = s.load_profile(a).unwrap().silver;

    // run end: no rows before -> rows and profile written
    s.pve_change_chests(a, 7, &[], &[row], |p| {
        p.silver += 10;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(s.pve_rewards(a, 7).unwrap(), vec![row]);
    assert_eq!(s.load_profile(a).unwrap().silver, silver + 10);

    // the same run end again: stale, nothing written
    let again = s.pve_change_chests(a, 7, &[], &[opened], |p| {
        p.silver += 10;
        Ok::<_, ()>(())
    });
    assert!(matches!(again, Err(UpdateError::Rejected(ChestRefusal::Stale))));
    assert_eq!(s.pve_rewards(a, 7).unwrap(), vec![row]);
    assert_eq!(s.load_profile(a).unwrap().silver, silver + 10);

    // a refused profile change writes neither the profile nor the rows
    let refused = s.pve_change_chests(a, 7, &[row], &[opened], |p| {
        p.silver += 99;
        Err::<(), _>("gold")
    });
    assert!(matches!(refused, Err(UpdateError::Rejected(ChestRefusal::Rejected("gold")))));
    assert_eq!(s.pve_rewards(a, 7).unwrap(), vec![row]);
    assert_eq!(s.load_profile(a).unwrap().silver, silver + 10);

    // an accepted change replaces the rows
    s.pve_change_chests(a, 7, &[row], &[opened], |p| {
        p.silver -= 5;
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(s.pve_rewards(a, 7).unwrap(), vec![opened]);
    assert_eq!(s.load_profile(a).unwrap().silver, silver + 5);
}

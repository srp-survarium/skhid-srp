//! Skills over the wire: 47 00 learn, 47 01 reset (paid), 8C with the list, points accounting, restart.

mod support;

use std::sync::Arc;

use game_data::GameData;
use lobby_server::{LobbyConfig, MinimalMatches};
use protocol::c2s::Request;
use protocol::s2c::*;
use support::*;

fn env() -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let e = Env::new(&dir.path().join("srp.sqlite"));
    (dir, e)
}

fn level8c(msgs: &[S2c]) -> Option<&Level8C> {
    msgs.iter().rev().find_map(|m| if let S2c::Level(l) = m { Some(l) } else { None })
}

fn money(msgs: &[S2c]) -> Option<&Money> {
    msgs.iter().rev().find_map(|m| if let S2c::Money(x) = m { Some(x) } else { None })
}

fn learn(e: &mut Env, c: &mut Client, v: &[(u16, u8)]) -> Vec<S2c> {
    c.send(Request::Skills { skills: v.to_vec() });
    e.pump(&mut [&mut *c]);
    c.take()
}

fn reset(e: &mut Env, c: &mut Client, branch: u8, payment: u8) -> Vec<S2c> {
    c.send(Request::SkillReset { branch, payment });
    e.pump(&mut [&mut *c]);
    c.take()
}

/// Level 5 (the starter profile: 5 points) unless `xp` is given.
fn client_with_xp(e: &mut Env, xp: u32) -> Client {
    let mut c = e.client("test");
    e.store.update_profile(c.account, |p| {
        p.xp = xp;
        Ok::<_, ()>(())
    })
    .unwrap();
    e.sign_in(&mut c);
    c
}

#[test]
fn login_8c_lists_the_stored_skills() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    e.store.update_profile(c.account, |p| {
        p.skills.insert(112, 2);
        p.skills.insert(513, 5);
        Ok::<_, ()>(())
    })
    .unwrap();
    let msgs = e.sign_in(&mut c);
    let l = level8c(&msgs).unwrap();
    assert_eq!(l.skills, vec![(112, 2), (513, 5)]);
    assert_eq!(l.level.points, 4, "unlocked points = level - 1 (level 5)");
}

#[test]
fn learn_flow_points_accounting() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = client_with_xp(&mut e, 2790);
    // level 6 = 5 points (one per level gained)
    let m = learn(&mut e, &mut c, &[(112, 3)]);
    // no 83 47 reply, only the list (no B9: 401 counts the fewest points of all branches, still 0 with one branch)
    assert_eq!(m.iter().map(|m| m.id()).collect::<Vec<_>>(), [0x8C]);
    assert_eq!(level8c(&m).unwrap().skills, vec![(112, 3)]);
    assert_eq!(e.store.load_profile(c.account).unwrap().skills.len(), 1);
    // the client sends the whole set again with one more skill
    let m = learn(&mut e, &mut c, &[(112, 3), (113, 2)]);
    assert_eq!(level8c(&m).unwrap().skills, vec![(112, 3), (113, 2)]);
    assert_eq!(e.store.load_profile(c.account).unwrap().skill_points_spent(), 5);
    // a sixth point, a taken-back point, an unknown skill: refused, the list is sent again unchanged
    for bad in [vec![(112, 3), (113, 2), (114, 1)], vec![(112, 3), (113, 2), (543, 1)], vec![(112, 1), (113, 2)], vec![(112, 3), (113, 2), (7, 1)]] {
        let m = learn(&mut e, &mut c, &bad);
        assert_eq!(level8c(&m).unwrap().skills, vec![(112, 3), (113, 2)], "{bad:?}");
    }
    assert_eq!(e.store.load_profile(c.account).unwrap().skills.len(), 2);
}

#[test]
fn prerequisites_need_points_in_the_branch() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let gd = game_data::testing::tables();
    // level 20 or more
    let mut c = client_with_xp(&mut e, 20_000);
    let lvl = gd.levels.level_fields(20_000).skill_points;
    assert!(lvl >= 10);
    // 121 needs 5 points in branch 1 first
    let m = learn(&mut e, &mut c, &[(121, 1)]);
    assert!(level8c(&m).unwrap().skills.is_empty());
    let m = learn(&mut e, &mut c, &[(112, 5), (121, 1)]);
    assert_eq!(level8c(&m).unwrap().skills, vec![(112, 5), (121, 1)]);
    // points of branch 2 do not unlock branch 1 skills of a higher tier (131 needs 10)
    let m = learn(&mut e, &mut c, &[(112, 5), (121, 1), (212, 5), (131, 1)]);
    assert_eq!(level8c(&m).unwrap().skills, vec![(112, 5), (121, 1)]);
}

#[test]
fn reset_each_branch_costs_money() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = client_with_xp(&mut e, 20_000);
    learn(&mut e, &mut c, &[(112, 5), (121, 1), (212, 2)]);
    let gd = game_data::testing::tables();
    let per = gd.skills.prices.reset_silver_per_point as u64;
    let p0 = e.store.load_profile(c.account).unwrap();
    // branch 0 (skills 1xx): 6 points, paid in silver
    let m = reset(&mut e, &mut c, 0, 0);
    assert_eq!(m.iter().map(|m| m.id()).collect::<Vec<_>>(), [0x8B, 0x8C]);
    assert_eq!(level8c(&m).unwrap().skills, vec![(212, 2)]);
    assert_eq!(money(&m).unwrap().silver as u64, p0.silver as u64 - 6 * per);
    assert_eq!(money(&m).unwrap().gold, p0.gold);
    // branch 4 holds nothing: free, no change
    let m = reset(&mut e, &mut c, 4, 0);
    assert_eq!(money(&m).unwrap().silver as u64, p0.silver as u64 - 6 * per);
    // branch 1 (skills 2xx), paid in gold
    let gper = gd.skills.prices.reset_gold_per_point;
    let m = reset(&mut e, &mut c, 1, 1);
    assert!(level8c(&m).unwrap().skills.is_empty());
    assert_eq!(money(&m).unwrap().gold, p0.gold - 2 * gper);
    let p1 = e.store.load_profile(c.account).unwrap();
    assert!(p1.skills.is_empty());
    // the points are free to spend again
    let m = learn(&mut e, &mut c, &[(212, 2)]);
    assert_eq!(level8c(&m).unwrap().skills, vec![(212, 2)]);
}

#[test]
fn reset_without_money_or_with_bad_bytes_changes_nothing() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = client_with_xp(&mut e, 2790);
    learn(&mut e, &mut c, &[(112, 5)]);
    e.store.update_profile(c.account, |p| {
        p.silver = 100;
        p.gold = 1;
        Ok::<_, ()>(())
    })
    .unwrap();
    for (branch, pay) in [(0u8, 0u8), (0, 1), (5, 0), (9, 0), (0, 7)] {
        let m = reset(&mut e, &mut c, branch, pay);
        assert_eq!(level8c(&m).unwrap().skills, vec![(112, 5)], "branch {branch} pay {pay}");
    }
    let p = e.store.load_profile(c.account).unwrap();
    assert_eq!((p.silver, p.gold), (100, 1));
}

#[test]
fn skills_persist_across_restart_and_accounts_are_separate() {
    game_data::require_tables!();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("srp.sqlite");
    let mut e = Env::new(&path);
    let mut a = e.client("alpha");
    let mut b = e.client("beta");
    for c in [&a, &b] {
        e.store.update_profile(c.account, |p| {
            p.xp = 2790; // level 6: five points
            Ok::<_, ()>(())
        })
        .unwrap();
    }
    e.sign_in(&mut a);
    e.sign_in(&mut b);
    learn(&mut e, &mut a, &[(112, 2), (513, 3)]);
    learn(&mut e, &mut b, &[(113, 1)]);
    e.restart(&path, LobbyConfig::default(), Arc::new(MinimalMatches));
    let mut a2 = e.client_of("alpha");
    let mut b2 = e.client_of("beta");
    let ma = e.sign_in(&mut a2);
    let mb = e.sign_in(&mut b2);
    assert_eq!(level8c(&ma).unwrap().skills, vec![(112, 2), (513, 3)]);
    assert_eq!(level8c(&mb).unwrap().skills, vec![(113, 1)]);
}

#[test]
fn skill_requests_before_sign_in_are_ignored_and_bad_bytes_do_not_crash() {
    game_data::require_tables!();
    let (_d, mut e) = env();
    let mut c = e.client("test");
    c.send(Request::Skills { skills: vec![(112, 1)] });
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    e.sign_in(&mut c);
    // a truncated 47 00 (n = 3 but one entry) is not decoded and gets no answer
    c.send_raw(0x47, &[0, 3, 0x70, 0, 1]);
    e.pump(&mut [&mut c]);
    assert!(c.take().is_empty());
    assert!(e.store.load_profile(c.account).unwrap().skills.is_empty());
}

#[test]
fn prestige_ranks_add_skill_points() {
    game_data::require_tables!();
    // one point per faction level above 5 (OFFICIAL 0.56): faction 2 at level 7 (500 000) adds 2 to the level points
    let (_d, mut e) = env();
    let gd = game_data::testing::tables();
    let base = u32::from(gd.levels.level_fields(20_000).skill_points);
    let mut c = e.client("test");
    e.store
        .update_profile(c.account, |p| {
            p.xp = 20_000;
            for f in 1..=4u8 {
                p.fac_points.insert(f, 0);
            }
            p.fac_points.insert(2, 500_000);
            Ok::<_, ()>(())
        })
        .unwrap();
    let m = e.sign_in(&mut c);
    let m2 = e.handshake(&mut c);
    let l = level8c(&m2).or(level8c(&m)).expect("8C");
    assert_eq!(u32::from(l.level.points), base + 2);
}

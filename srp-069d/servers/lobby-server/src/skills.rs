//! Skills: apply 47 00 (learn) and reset 47 01 (unlearn, paid). Rules in `game_data::skills`.
//!
//! The client has no `83 47` reply case: the skill list comes back in 8C (`u8 n, n x {u16 id, u8 points}`), a reset
//! also changes the money (8B). Every request is answered with 8C (and 8B), also a refused one, so the window shows
//! the stored state again.

use game_data::skills::{ResetPayment, Skills};
use protocol::s2c::*;

use crate::build;
use crate::dispatch::{update, Ctx};
use crate::peer::Peer;
use accounts::Currency;

/// 47 00 `u8 n, n x {u16 id, u8 points}`: the whole learned set after the player's Apply.
pub fn apply(cx: &mut Ctx, peer: &mut Peer, requested: &[(u16, u8)]) {
    let gd = &cx.lobby.data;
    let res = update(cx, peer, |p| {
        let avail = gd.skills.points_for_level(build::skill_points(gd, p));
        let new = gd.skills.apply(&p.skills, requested, avail)?;
        p.skills = new;
        Ok::<u32, game_data::SkillError>(p.skill_points_spent())
    });
    match res {
        Some(Ok(spent)) => log::info!("{}: 47 00 skills learned: {} skills, {spent} points", peer.addr, requested.len()),
        Some(Err(e)) => log::info!("{}: 47 00 refused: {e}", peer.addr),
        None => {}
    }
    resync(cx, peer, false);
}

/// 47 01 `u8 branch (0..4 = id / 100 - 1), u8 payment (0 silver, 1 gold)`: take the points of a branch back for money.
pub fn reset(cx: &mut Ctx, peer: &mut Peer, branch: u8, payment: u8) {
    let gd = &cx.lobby.data;
    let res = update(cx, peer, |p| {
        let pay = Skills::payment(payment)?;
        let points = gd.skills.reset_points(&p.skills, branch)?;
        if points == 0 {
            return Ok((0, 0));
        }
        let cost = gd.skills.reset_cost(points, pay);
        let cur = if pay == ResetPayment::Gold { Currency::Gold } else { Currency::Silver };
        if p.spend(cur, cost).is_err() {
            return Err(game_data::SkillError::NoMoney);
        }
        p.skills = gd.skills.without_branch(&p.skills, branch);
        Ok::<(u32, u64), game_data::SkillError>((points, cost))
    });
    match res {
        Some(Ok((0, _))) => log::info!("{}: 47 01 branch {branch}: nothing learned", peer.addr),
        Some(Ok((points, cost))) => log::info!("{}: 47 01 branch {branch} reset: {points} points back for {cost} (payment {payment})", peer.addr),
        Some(Err(e)) => log::info!("{}: 47 01 refused: {e}", peer.addr),
        None => {}
    }
    resync(cx, peer, true);
}

fn resync(cx: &mut Ctx, peer: &mut Peer, money: bool) {
    let Some(id) = peer.account else { return };
    let Ok(p) = cx.lobby.store.load_profile(id) else { return };
    if money {
        peer.push(&S2c::Money(build::money_8b(&p)));
    }
    peer.push(&S2c::Level(build::level_8c(&cx.lobby.data, &p)));
    crate::achievements::after_skills(cx, peer);
}

//! Clans on the lobby: 48 07..17 / 26, 4D kind 7, the 9A pushes and the clan chat channel. Storage and rules are in
//! `accounts::clans`; this module turns them into messages. Layouts: `protocol::s2c::clans`.
//!
//! Flow (the parts marked PROVEN are read from the client, docs/notes/social.md has the rest):
//! * name / abbreviation check `48 09` / `48 0A` -> `83 48 09|0A str, u8 ok [, u8 reason]` (PROVEN 0x19223E);
//! * `48 07` reserve (name, abbreviation), `48 0D` pay the registration, `48 08` create: the reservation and the
//!   payment are stored; creation needs both (choice: the order of the three buttons in the UI is not proven, so the
//!   server accepts them in the natural order and refuses with a client-shown text otherwise);
//! * `48 10` invite -> `83 48 10` makes the client send `4D kind 7` (PROVEN 0x1926E4), which the server turns into a
//!   B1 message of type 7 for the target; accepting sends `48 0B` (PROVEN);
//! * every change pushes the full clan (9A sub 0) to every online member; the player who left gets 9A "no clan".

use accounts::clans::{self as rules, Clan, ClanError};
use accounts::{account_of_public_id, public_id, AccountId};
use protocol::c2s::Account48;
use protocol::s2c::*;
use protocol::Encode;

use crate::dispatch::Ctx;
use crate::peer::{Cross, Peer};

/// Registration price in silver (choice: the client has the slot `set_clan_creation_cost` in 8D and the game data
/// has no value; the same number is sent in 8D so the UI shows what is charged).
pub const CREATION_COST_SILVER: u32 = 50_000;

fn raw(m: Clan9A) -> S2c {
    S2c::Raw { id: 0x9A, payload: m.to_bytes().unwrap_or_default() }
}

fn denied(text: &str) -> S2c {
    S2c::Denied(Denied { request: 0x48, body: DeniedBody::Text(text.into()) })
}

fn members_of(cx: &Ctx, clan: &Clan) -> Vec<Member9A> {
    clan.members
        .iter()
        .map(|m| {
            let level = cx.lobby.store.load_profile(m.id).map_or(1, |p| cx.lobby.data.levels.level_of(p.xp));
            Member9A { id: public_id(m.id), name: m.name.clone(), rank: m.rank, level: level.min(255) as u8, online: cx.registry.is_online(m.id) }
        })
        .collect()
}

fn ranks_of(clan: &Clan) -> Vec<Rank9A> {
    clan.ranks.iter().map(|r| Rank9A { name: r.name.clone(), perms: r.perms, default: r.standard }).collect()
}

/// 9A sub 0 as `viewer` sees it (the `u16` is the viewer's own index in the member list).
pub fn full_msg(cx: &Ctx, clan: &Clan, viewer: AccountId) -> S2c {
    let members = members_of(cx, clan);
    let me_index = clan.members.iter().position(|m| m.id == viewer).unwrap_or(0) as u16;
    // level and bar from the stored clan experience (game_data::clan: CHOICE table, PROVEN bar fields)
    let pr = game_data::clan::progress(clan.xp);
    raw(Clan9A::Full {
        clan_id: clan.id,
        name: clan.name.clone(),
        abbr: clan.abbr.clone(),
        ranks: ranks_of(clan),
        commander: clan.member(viewer).map(|m| m.rank) == Some(0),
        members,
        me_index,
        level: pr.level,
        players: clan.members.len() as u32,
        min: pr.min,
        max: pr.max,
        current: pr.current,
    })
}

/// 9A for the login pushes and `48 26`: the clan of the account, or "no clan".
/// The registration form of the account's reservation (9A sub 0, docs/notes/social.md "Clan registration"): the founder alone
/// (no supporters are required: P = 1, the 9C counter needs 0), the silver collected so far, paid when it reaches the
/// cost, name state 3 (the names were checked when reserved). None without a reservation.
fn registration_msg(cx: &Ctx, me: AccountId) -> Option<S2c> {
    let (name, abbr, paid) = cx.lobby.store.clan_reservation(me).ok().flatten()?;
    let collected = cx.lobby.store.clan_registration_collected(me).unwrap_or(0);
    let nick = cx.lobby.store.account(me).ok().flatten().map(|a| a.name).unwrap_or_default();
    Some(raw(Clan9A::Registration {
        clan_id: me as u32,
        name,
        abbr,
        ranks: vec![],
        members: vec![(public_id(me), nick, 0, 0)],
        me_index: 0,
        commander: true,
        supporters_ok: true,
        paid,
        collected: if paid { collected.max(CREATION_COST_SILVER) } else { collected },
        name_state: 3,
    }))
}

pub fn state_msg(cx: &Ctx, me: AccountId) -> S2c {
    match cx.lobby.store.clan_of(me) {
        Ok(Some(c)) => full_msg(cx, &c, me),
        Ok(None) => registration_msg(cx, me).unwrap_or_else(|| raw(Clan9A::NoClan)),
        Err(e) => {
            log::warn!("clan of {me}: {e}");
            raw(Clan9A::NoClan)
        }
    }
}

fn send(cx: &mut Ctx, peer: &mut Peer, to: AccountId, msg: S2c) {
    if peer.account == Some(to) {
        peer.push(&msg);
    } else if cx.registry.is_online(to) {
        cx.fx.push(Cross { to, msg });
    }
}

/// Push the full clan to every online member.
fn refresh(cx: &mut Ctx, peer: &mut Peer, clan_id: u32) {
    let Ok(Some(clan)) = cx.lobby.store.load_clan(clan_id) else { return };
    for m in &clan.members {
        if cx.registry.is_online(m.id) || peer.account == Some(m.id) {
            let msg = full_msg(cx, &clan, m.id);
            send(cx, peer, m.id, msg);
        }
    }
}

/// A finished match adds experience to the member's clan (`game_data::clan::match_xp`); every online member gets the
/// refreshed clan window (9A sub 0: level, bar). Called from `stats::record_match` with the facts of the match.
pub fn record(cx: &mut Ctx, account: AccountId, facts: &game_data::MatchFacts) {
    let Some(gain) = game_data::clan::match_xp(facts) else { return };
    let store = &cx.lobby.store;
    let Ok(Some(cid)) = store.clan_id_of(account) else { return };
    let Ok(Some((old, new))) = store.clan_add_xp(cid, gain) else { return };
    let (a, b) = (game_data::clan::progress(old).level, game_data::clan::progress(new).level);
    log::info!("clan {cid}: +{gain} experience from account {account} ({old} -> {new}{})", if b > a { format!(", level {b}") } else { String::new() });
    let Ok(Some(clan)) = store.load_clan(cid) else { return };
    for m in clan.members.iter().filter(|m| cx.registry.is_online(m.id)) {
        let msg = full_msg(cx, &clan, m.id);
        cx.fx.push(Cross { to: m.id, msg });
    }
}

/// A member went online or offline: the clan sees the member list again (9A sub 2).
pub fn presence_changed(cx: &mut Ctx, account: AccountId) {
    let Ok(Some(clan)) = cx.lobby.store.clan_of(account) else { return };
    for m in clan.members.iter().filter(|m| m.id != account && cx.registry.is_online(m.id)) {
        let msg = raw(Clan9A::Members {
            clan_id: clan.id,
            members: members_of(cx, &clan),
            me_index: clan.members.iter().position(|x| x.id == m.id).unwrap_or(0) as u16,
            commander: m.rank == 0,
        });
        cx.fx.push(Cross { to: m.id, msg });
    }
}

/// Invites waiting for the account become B1 messages at sign-in (after the lobby loaded).
pub fn pending_invites(cx: &mut Ctx, peer: &mut Peer) {
    let Some(me) = peer.account else { return };
    for (cid, inviter) in cx.lobby.store.clan_invites_of(me).unwrap_or_default() {
        if let Some(m) = invite_message(cx, cid, inviter) {
            peer.push(&m);
        }
    }
}

fn invite_message(cx: &Ctx, clan: u32, inviter: AccountId) -> Option<S2c> {
    let c = cx.lobby.store.load_clan(clan).ok()??;
    let name = cx.lobby.store.account(inviter).ok()??.name;
    // the message id only has to be unique per message of the player: clan id and inviter give a stable one
    let message_id = clan.wrapping_mul(1_000_003) ^ (inviter as u32);
    Some(S2c::SystemMessage(SystemMessage { kind: 7, sender: public_id(inviter), name, clan: c.abbr, flag: 0, text: c.name, message_id }))
}

fn validate(cx: &Ctx, me: AccountId, name: Option<&str>, abbr: Option<&str>) -> Result<(), u8> {
    if let Some(n) = name {
        rules::check_clan_name(n)?;
    }
    if let Some(a) = abbr {
        rules::check_clan_abbr(a)?;
    }
    if cx.lobby.store.clan_text_taken(name, abbr, me).unwrap_or(true) {
        return Err(rules::R_EXISTS);
    }
    Ok(())
}

fn check_reply(sub: u8, text: &str, result: Result<(), u8>) -> Vec<u8> {
    let mut b = vec![sub, text.len().min(100) as u8];
    b.extend(&text.as_bytes()[..text.len().min(100)]);
    match result {
        Ok(()) => b.push(1),
        Err(reason) => b.extend([0, reason]),
    }
    b
}

/// The `84 48` text of a clan refusal: a client localization key (every language).
/// `ClanError::Rule` carries its key from the accounts crate.
fn why(e: &ClanError) -> String {
    match e {
        ClanError::NotInClan => "st_no_clan_info".into(),
        ClanError::AlreadyInClan => "st_player_is_already_in_clan".into(),
        ClanError::Taken => "st_clan_name_and_abbr_are_not_valid".into(),
        ClanError::InvalidName(_) => "st_string_pattern_error".into(),
        ClanError::NoReservation => "st_cant_create_clan".into(),
        ClanError::NotPaid => "st_cant_create_clan".into(),
        ClanError::Denied => "st_dont_have_permission".into(),
        ClanError::Full => "st_clan_max_members_count_error".into(),
        ClanError::NoSuchMember => "st_player_not_found".into(),
        ClanError::NoInvite => "st_cant_add_clan_member".into(),
        ClanError::Rule(t) => (*t).into(),
        ClanError::NoSuchClan => "st_cant_add_clan_member".into(),
        ClanError::Store(e) => {
            log::error!("clan store: {e}");
            "st_error_during_storing_user_comment".into()
        }
    }
}

fn fail(peer: &mut Peer, e: &ClanError) {
    log::info!("{}: clan request refused: {e}", peer.addr);
    peer.push(&denied(&why(e)));
}

/// A member left / was removed: they see "no clan"; the others the new member list.
fn removed(cx: &mut Ctx, peer: &mut Peer, who: AccountId, clan_id: u32) {
    send(cx, peer, who, raw(Clan9A::NoClan));
    refresh(cx, peer, clan_id);
}

/// `48 xx` clan requests. Returns false for subs that are not clan requests.
pub fn handle(cx: &mut Ctx, peer: &mut Peer, a: &Account48) -> bool {
    let Some(me) = peer.account else { return false };
    let store = cx.lobby.store.clone();
    match a {
        Account48::ClanNameCheck { name } => {
            let r = validate(cx, me, Some(name), None);
            peer.reply_raw(0x48, &check_reply(9, name, r));
        }
        Account48::ClanAbbrCheck { abbr } => {
            let r = validate(cx, me, None, Some(abbr));
            peer.reply_raw(0x48, &check_reply(0x0A, abbr, r));
        }
        Account48::ReserveClanName { name, abbr } => match store.reserve_clan(me, name, abbr) {
            Ok(()) => {
                peer.reply_raw(0x48, &[7]);
                peer.push(&raw(Clan9A::Reservation { clan_id: me as u32, name: name.clone(), abbr: abbr.clone() }));
                // the registration form moves the creation window to its payment step (without it the window stays at
                // the reserve step: 0x20018C2C8..)
                if let Some(m) = registration_msg(cx, me) {
                    peer.push(&m);
                }
            }
            Err(e) => fail(peer, &e),
        },
        // 48 0D: a deposit of the silver the player chose (the window's stepper, at most cost - collected): taken with the
        // running total in one transaction; the form shows the new total (the client reads nothing from 83 48 0D)
        Account48::BuyClanRegistration { amount } => match store.deposit_clan_registration(me, *amount, CREATION_COST_SILVER) {
            Ok((taken, collected, paid)) => {
                log::info!("{}: 48 0D deposited {taken} of {amount}: {collected} / {CREATION_COST_SILVER} collected, paid {paid}", peer.addr);
                if let Ok(p) = store.load_profile(me) {
                    peer.push(&S2c::Money(crate::build::money_8b(&p)));
                }
                peer.reply_raw(0x48, &[0x0D]);
                if let Some(m) = registration_msg(cx, me) {
                    peer.push(&m);
                }
            }
            Err(e) => fail(peer, &e),
        },
        // 48 0C: the invitation went out; the client prints "Player <name> successfully invited to clan."
        Account48::ClanInviteSent { player } => {
            let name = account_of_public_id(*player).and_then(|t| store.account(t).ok().flatten()).map(|a| a.name).unwrap_or_default();
            let mut b = vec![0x0C];
            b.extend(player.to_le_bytes());
            b.push(name.len().min(127) as u8);
            b.extend(&name.as_bytes()[..name.len().min(127)]);
            peer.reply_raw(0x48, &b);
        }
        Account48::CreateClan => match store.create_clan(me) {
            Ok(id) => {
                log::info!("{}: clan {id} created", peer.addr);
                peer.reply_raw(0x48, &[8]);
                refresh(cx, peer, id);
            }
            Err(e) => fail(peer, &e),
        },
        Account48::ClanData => {
            let m = state_msg(cx, me);
            peer.push(&m);
        }
        Account48::ClanInvite { player } => {
            let Some(target) = account_of_public_id(*player) else { return true };
            match store.clan_invite(me, target) {
                Ok(cid) => {
                    let clan = store.load_clan(cid).ok().flatten();
                    let tname = store.account(target).ok().flatten().map(|a| a.name).unwrap_or_default();
                    let mut b = vec![0x10, 0, 0, 0, 0, 0];
                    b.extend(player.to_le_bytes());
                    b.push(tname.len().min(127) as u8);
                    b.extend(&tname.as_bytes()[..tname.len().min(127)]);
                    b.extend([0u8; 8]);
                    let text = clan.map(|c| c.name).unwrap_or_default();
                    b.push(text.len().min(127) as u8);
                    b.extend(&text.as_bytes()[..text.len().min(127)]);
                    peer.reply_raw(0x48, &b);
                }
                Err(e) => fail(peer, &e),
            }
        }
        Account48::ImportantMessageRead { id, answer } => answer_invite(cx, peer, *id, *answer),
        // 48 0F: the clan window (or its members tab) opened: nothing changes; the current clan state is sent
        Account48::ClanWindowOpened { .. } => {
            let m = state_msg(cx, me);
            peer.push(&m);
        }
        // 48 0E: the confirmed leave / disband / cancel-registration dialog; the bytes are the same for the three, so the
        // sender's role decides (CHOICE): no clan -> cancel the reservation, commander -> disband, member -> leave
        Account48::LeaveClan => match store.clan_of(me) {
            Ok(None) => {
                let _ = store.cancel_clan_reservation(me);
                peer.push(&raw(Clan9A::NoClan));
            }
            Ok(Some(c)) if c.member(me).is_some_and(|m| m.rank == 0) => match store.clan_disband(me) {
                Ok((_, former)) => {
                    for m in former {
                        send(cx, peer, m, raw(Clan9A::Disbanded));
                        send(cx, peer, m, raw(Clan9A::NoClan));
                    }
                }
                Err(e) => fail(peer, &e),
            },
            Ok(Some(_)) => match store.clan_leave(me) {
                Ok((cid, rest, disbanded)) => {
                    if disbanded {
                        send(cx, peer, me, raw(Clan9A::NoClan));
                    } else {
                        removed(cx, peer, me, cid);
                        let _ = rest;
                    }
                }
                Err(e) => fail(peer, &e),
            },
            Err(e) => fail(peer, &e),
        },
        // 48 14: the confirmed kick (dialog 0xDB)
        Account48::Kick { member } => {
            let Some(target) = account_of_public_id(*member) else { return true };
            match store.clan_kick(me, target) {
                Ok(cid) => removed(cx, peer, target, cid),
                Err(e) => fail(peer, &e),
            }
        }
        Account48::ClanPermission { rank, value, permission } => {
            // the client's permission index follows the callback order invite, kick, change rank, daily message,
            // battle commander (LIKELY); the mask bit is 4 - index (the unpack order of the mask)
            let bit = 4u8.saturating_sub(*permission);
            match store.clan_set_perm(me, *rank, bit, *value != 0) {
                Ok(cid) => refresh(cx, peer, cid),
                Err(e) => fail(peer, &e),
            }
        }
        Account48::RankDown { member, a } => {
            let Some(target) = account_of_public_id(*member) else { return true };
            // 48 12 member 1 = the confirmed hand-over of command (after `83 48 13 01`); 0 = rank down
            if *a == 1 {
                let deputy = store.clan_of(me).ok().flatten().is_some_and(|c| c.member(me).is_some_and(|m| m.rank == 0) && c.member(target).is_some_and(|m| m.rank == 1));
                if !deputy {
                    fail(peer, &ClanError::Denied);
                    return true;
                }
                match store.clan_move_rank(me, target, true) {
                    Ok((cid, _)) => refresh(cx, peer, cid),
                    Err(e) => fail(peer, &e),
                }
            } else {
                match store.clan_move_rank(me, target, false) {
                    Ok((cid, _)) => refresh(cx, peer, cid),
                    Err(e) => fail(peer, &e),
                }
            }
        }
        Account48::RankUp { member } => {
            let Some(target) = account_of_public_id(*member) else { return true };
            // a move to rank 0 hands command over: the client must ask first (`83 48 13 01` shows "Are you sure ...";
            // its Yes sends 48 12 member 1): nothing changes now
            let to_commander = store.clan_of(me).ok().flatten().is_some_and(|c| c.member(me).is_some_and(|m| m.rank == 0) && c.member(target).is_some_and(|m| m.rank == 1));
            if to_commander {
                peer.reply_raw(0x48, &[0x13, 1]);
                return true;
            }
            match store.clan_move_rank(me, target, true) {
                Ok((cid, handover)) => {
                    peer.reply_raw(0x48, &[0x13, u8::from(handover)]);
                    refresh(cx, peer, cid);
                }
                Err(e) => fail(peer, &e),
            }
        }
        Account48::CreateRank { name } => match store.clan_create_rank(me, name) {
            Ok(cid) => refresh(cx, peer, cid),
            Err(e) => fail(peer, &e),
        },
        Account48::RemoveRank { rank } => match store.clan_remove_rank(me, *rank) {
            Ok(cid) => refresh(cx, peer, cid),
            Err(e) => fail(peer, &e),
        },
        Account48::DailyMessage { text } => match store.clan_set_daily_message(me, text) {
            Ok(cid) => {
                if let Ok(Some(c)) = store.load_clan(cid) {
                    for m in &c.members {
                        send(cx, peer, m.id, raw(Clan9A::Daily { clan_id: cid, text: c.daily_message.clone() }));
                    }
                }
            }
            Err(e) => fail(peer, &e),
        },
        _ => return false,
    }
    true
}

/// 48 0B: the answer to a clan invitation. `id` is the message's sender (the inviter, LIKELY); with a single open
/// invitation it is taken whatever the id says.
fn answer_invite(cx: &mut Ctx, peer: &mut Peer, id: u64, answer: u8) {
    let Some(me) = peer.account else { return };
    let open = cx.lobby.store.clan_invites_of(me).unwrap_or_default();
    let inviter = account_of_public_id(id);
    let pick = open.iter().find(|(_, i)| Some(*i) == inviter).or(if open.len() == 1 { open.first() } else { None }).copied();
    let Some((cid, _)) = pick else {
        log::debug!("{}: 48 0B without a clan invitation", peer.addr);
        return;
    };
    // 0 = accept, 1 = decline (PROVEN: 0x2001E88E0 sends the Flash answer unchanged in 48 0B; NotificationInvite accept = 0)
    if answer == 0 {
        match cx.lobby.store.clan_accept(me, cid) {
            Ok(()) => {
                log::info!("{}: joined clan {cid}", peer.addr);
                refresh(cx, peer, cid);
            }
            Err(e) => fail(peer, &e),
        }
    } else {
        let _ = cx.lobby.store.clan_decline(me, cid);
    }
}

/// 4D kind 7 (sent by the inviter's client after `83 48 10`): deliver the invitation to the target. Only an
/// invitation the clan store holds is delivered.
pub fn invite_relay(cx: &mut Ctx, peer: &mut Peer, target: u64) {
    let Some(me) = peer.account else { return };
    let Some(to) = account_of_public_id(target) else { return };
    let held = cx.lobby.store.clan_invites_of(to).unwrap_or_default().into_iter().find(|(_, i)| *i == me);
    let Some((cid, _)) = held else {
        log::info!("{}: 4D 7 for {to} without a stored invitation", peer.addr);
        return;
    };
    if let (true, Some(m)) = (cx.registry.is_online(to), invite_message(cx, cid, me)) {
        cx.fx.push(Cross { to, msg: m });
    }
}

/// Online members of the sender's clan except the sender (chat channel 3).
pub fn chat_audience(cx: &Ctx, me: AccountId, clan_id: u64) -> Vec<AccountId> {
    match cx.lobby.store.clan_of(me) {
        Ok(Some(c)) if u64::from(c.id) == clan_id => c.members.iter().map(|m| m.id).filter(|a| *a != me && cx.registry.is_online(*a)).collect(),
        _ => vec![],
    }
}

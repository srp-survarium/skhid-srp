//! `38 00 u8, u64 player, u32 match id` (click on a player row of the results screen) -> push `93` (docs/notes/social.md).
//!
//! The 93 carries the player's real stored account body; the client shows only that (`protocol::s2c::details`): the
//! clicked player's equipment view (`ProfilePlayerInfo`) with one tab per profile it may play in a match
//! ([`build::account_body_93`]). A request is answered for the requester's own account
//! and for squad mates (CHOICE, the same rule as the `33` member profile), and for the server bots of the requester's
//! last bot-filled PvP result (their match kit, [`build::bot_body_93`]); other ids get no answer.

use accounts::{account_of_public_id, public_id};
use protocol::s2c::*;
use protocol::Encode;

use crate::build;
use crate::dispatch::Ctx;
use crate::peer::Peer;

pub fn player(cx: &mut Ctx, peer: &mut Peer, target: u64, match_id: u32) {
    let Some(me) = peer.account else { return };
    let Some(t) = account_of_public_id(target) else {
        bot(cx, peer, me, target, match_id);
        return;
    };
    let allowed = t == me || cx.registry.squads.squad_of(me).is_some_and(|s| s.members.iter().any(|x| x.account == t));
    if !allowed {
        log::info!("{}: 38 00 for account {t}: not self or a squad mate, ignored", peer.addr);
        return;
    }
    let (Ok(p), Ok(Some(acc))) = (cx.lobby.store.load_profile(t), cx.lobby.store.account(t)) else { return };
    let body = build::account_body_93(&cx.lobby.data, &p, &acc.name);
    log::info!("{}: 38 00 for account {t}: 93 with {} playable profile(s) of 3", peer.addr, body.profiles.iter().filter(|w| w.kind == 0).count());
    push93(peer, public_id(t), body);
}

/// A row id below the public-id prefix: a server bot of the requester's last bot-filled PvP result (the `97` lists
/// them with ids 1.., online.rs `results97`). The 38's `u32` is the window's `match_id` (AS
/// `VictoryScreenPlayersStats.requestPlayerInfo`: `victory_screen_request_player_data(public_account_id, data.match_id)`).
fn bot(cx: &mut Ctx, peer: &mut Peer, me: accounts::AccountId, target: u64, match_id: u32) {
    let found = cx.registry.online.result_bots(me).filter(|(id, _)| *id == match_id).and_then(|(_, w)| {
        let k = usize::try_from(target).ok()?.checked_sub(1)?;
        w.bots.get(k).map(|b| (k, *b))
    });
    let Some((k, b)) = found else {
        log::info!("{}: 38 00 for {target:#x} (match {match_id:#x}): not a player account nor a bot of the last result, ignored", peer.addr);
        return;
    };
    let language = cx.registry.language(me);
    let name = b.names.and_then(|(f, s)| cx.lobby.data.bot_names.name(usize::from(f), usize::from(s), language)).unwrap_or_else(|| format!("Bot {}", k + 1));
    let Some(body) = b.ai.and_then(|ai| build::bot_body_93(&cx.lobby.data, usize::from(ai), &name)) else {
        log::info!("{}: 38 00 for bot {target} of match {match_id:#x}: its AI player is not known, ignored", peer.addr);
        return;
    };
    log::info!("{}: 38 00 for bot {target} of match {match_id:#x}: 93 with the kit of AI player {:?}", peer.addr, b.ai);
    push93(peer, target, body);
}

fn push93(peer: &mut Peer, id: u64, body: protocol::items::AccountBody) {
    let msg = PlayerDetails93 { id, body };
    match msg.to_bytes() {
        Ok(payload) => peer.push(&S2c::Raw { id: 0x93, payload }),
        Err(e) => log::error!("{}: 93 encode: {e}", peer.addr),
    }
}

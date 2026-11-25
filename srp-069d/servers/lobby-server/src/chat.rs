//! Chat channels beyond general (1) and private (4): `4C u8 channel, u32 context id, str text`
//! (PROVEN, sender 0x2002641E0).
//!
//! The context id is a number the client got from the server and sends back (client fields +0xA8..+0xBC, set by the
//! pushes): 3 clan = the clan id of 9A (0x18BF8C), 8 squad = the squad id of AE, 9 custom match = the room id of
//! 98 (0x18BA9F), 5 / 6 / 7 = the match context set when a match is joined (0x18DFFA; the lobby sets it when it sends
//! `82`, see online.rs and [`crate::peer::Registry::set_match_context`]). The id is checked against the server's own
//! membership, so a stale or forged id reaches nobody. The sender never gets the line back (the client prints it
//! itself, 0x2643E0). Channel 2 is the system channel: only the server writes to it.

use accounts::AccountId;

use crate::dispatch::Ctx;

/// Who gets a line of `channel` sent by `me` with context id `target` (never `me`).
pub fn audience(cx: &Ctx, me: AccountId, channel: u8, target: u64) -> Vec<AccountId> {
    match channel {
        3 => crate::clans::chat_audience(cx, me, target),
        8 => match cx.registry.squads.squad_of(me) {
            Some(s) if u64::from(s.id) == target => s.members.iter().map(|m| m.account).filter(|a| *a != me).collect(),
            _ => vec![],
        },
        9 => crate::rooms::chat_audience(cx, me, target),
        5 | 6 | 7 => match cx.registry.match_context(me) {
            Some((ctx, team)) if u64::from(ctx) == target => cx
                .registry
                .match_members(ctx)
                .into_iter()
                .filter(|(a, t)| *a != me && (channel == 5 || *t == team))
                .map(|(a, _)| a)
                .collect(),
            _ => vec![],
        },
        _ => vec![],
    }
}

//! `48 06` complaint / compliment about a player (`comment_about_user`, client 0x18D870 / reply reader 0x19253E).
//! Layout, rules and what is open: `docs/notes/social.md` section "Complaints and compliments".

use accounts::comments::{CommentResult, KIND_COMPLAINT};

use protocol::s2c::{Denied, DeniedBody, S2c};

use crate::dispatch::Ctx;
use crate::peer::Peer;

/// Store the comment and answer `83 48 06 <kind>` (the client shows "Complaint sent successfully." for kind 1,
/// "Praise sent successfully." otherwise). A refusal is `84 x 48 01 <key>` (PROVEN channel: the client localises the key
/// and shows it as a system message, 0x200176400 / 0x200176E3A). The keys exist only in the localisation: LIKELY the
/// original server's. `a` = reason, `b` = match id (0 from the lobby), PROVEN (`comment_about_user`).
pub fn file(cx: &mut Ctx, peer: &mut Peer, player: u64, kind: u8, a: u8, b: u32) {
    let Some(me) = peer.account else { return };
    let Some(target) = accounts::account_of_public_id(player) else {
        log::info!("{}: 48 06 about {player:#x}: not a player", peer.addr);
        return;
    };
    let kind = if kind == KIND_COMPLAINT { KIND_COMPLAINT } else { 0 };
    let max = if kind == KIND_COMPLAINT { cx.lobby.cfg.claims_per_day } else { cx.lobby.cfg.compliments_per_day };
    match cx.lobby.store.add_comment(me, target, kind, a, b, max) {
        Ok(CommentResult::Added) => {
            log::info!("{}: 48 06 kind {kind} about account {target} stored", peer.addr);
            peer.reply_raw(0x48, &[6, kind]);
        }
        Ok(r) => {
            log::info!("{}: 48 06 kind {kind} about account {target} refused: {r:?}", peer.addr);
            if let Some(key) = refusal_key(kind, r) {
                peer.push(&refused(key));
            }
        }
        Err(e) => {
            log::error!("{}: 48 06 not stored: {e}", peer.addr);
            peer.push(&refused("st_error_during_storing_user_comment"));
        }
    }
}

/// The localisation key of a refusal (`None`: no text fits; the client cannot pick such a target).
fn refusal_key(kind: u8, r: CommentResult) -> Option<&'static str> {
    let complaint = kind == KIND_COMPLAINT;
    match r {
        CommentResult::SameUserToday if complaint => Some("st_claims_count_per_user_owerflow"),
        CommentResult::SameUserToday => Some("st_compliment_count_per_user_owerflow"),
        CommentResult::DayLimit if complaint => Some("st_claims_count_per_day_owerflow"),
        CommentResult::DayLimit => Some("st_compliment_count_per_day_owerflow"),
        CommentResult::Added | CommentResult::NoTarget => None,
    }
}

fn refused(key: &str) -> S2c {
    S2c::Denied(Denied { request: 0x48, body: DeniedBody::Text(key.into()) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_refusal_has_its_own_text() {
        assert_eq!(refusal_key(KIND_COMPLAINT, CommentResult::SameUserToday), Some("st_claims_count_per_user_owerflow"));
        assert_eq!(refusal_key(0, CommentResult::SameUserToday), Some("st_compliment_count_per_user_owerflow"));
        assert_eq!(refusal_key(KIND_COMPLAINT, CommentResult::DayLimit), Some("st_claims_count_per_day_owerflow"));
        assert_eq!(refusal_key(0, CommentResult::DayLimit), Some("st_compliment_count_per_day_owerflow"));
        assert_eq!(refusal_key(0, CommentResult::NoTarget), None);
    }
}

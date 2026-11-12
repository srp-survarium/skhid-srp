//! The match removal callback reaches the durable account penalty owner.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use lobby_server::v026g::Inbox;
use match_server::v026g::{IdleRemoved, ResultsSink};
use session::v026g::{GameAccounts, rewards::Results};
use session::{SessionStore, SignInError};
use srp::v026g::Settler;

#[test]
fn removal_events_from_different_matches_apply_account_bans() {
    let store = Arc::new(SessionStore::new());
    let account_id = store.sign_in("idle", b"pw").unwrap();
    let other_id = store.sign_in("other", b"pw").unwrap();
    let mut settler = Settler::new(
        store.clone(),
        Arc::new(GameAccounts::new()),
        Arc::new(Results::new()),
        Inbox::new(),
    );
    for kick in 1..=6 {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        settler.idle_removed(IdleRemoved {
            match_id: kick,
            account_id,
            name: "idle".into(),
        });
        let seconds = match kick {
            3 => Some(1800),
            6 => Some(21_600),
            _ => None,
        };
        if let Some(seconds) = seconds {
            let ban = store.account_ban(account_id).unwrap();
            assert!(ban.until.unwrap() >= before + seconds);
            assert!(ban.until.unwrap() <= before + seconds + 5);
            assert_eq!(store.sign_in("idle", b"pw"), Err(SignInError::Banned));
            // Allow the next matches immediately, without waiting out a
            // real ban. The daily count must survive this admin action.
            assert!(store.unban_account(account_id));
        } else {
            assert!(store.account_ban(account_id).is_none());
        }
        assert_eq!(store.sign_in("other", b"pw"), Ok(other_id));
    }
}

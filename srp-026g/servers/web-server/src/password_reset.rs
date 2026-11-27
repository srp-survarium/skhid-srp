//! The admin's password reset (`docs/notes/password-reset.md`): a player
//! who forgot their password asks the owner, who clears it here while srp
//! runs; the player's next sign-in, with any password, sets the new one
//! (`SessionStore::reset_password`, the owner's decision of 2026-10-18).
//!
//! - The page's Password tab: the user name the player signs in with (not
//!   the character name), then a confirm step showing the account it
//!   matched, then the reset.
//! - A script's reset: `tab=reset_password&login=<user name>` with the
//!   token, as release.sh sends its maintenance (curl from the owner's
//!   server, whose address is in `SRP_ADMIN_IPS`). No confirm step; one line
//!   of text back: `ok <login> <account id>` (200), `no such login` (404),
//!   `no login` (400), `wrong token` (403), `locked: too many wrong tokens`
//!   (429), `pending <login> <account id>` (202: cleared, the saver writes it
//!   within seconds), `error: …` (500: cleared in memory, not written). An
//!   address not in the list, or a page without a token, gets the plain "not
//!   found" as for every admin request.

use std::net::IpAddr;

use session::{LockExt, PasswordReset, ResetWrite};

use super::{ContentType, Lang, Request, Response, TokenRefused, Web, clock_utc, date_utc, escape};

/// The tab a script names for the reset without the confirm step.
pub(crate) const DIRECT_TAB: &str = "reset_password";

/// What the password tab shows.
#[derive(Default)]
pub(crate) struct ResetView {
    /// The user name in the field.
    typed: String,
    /// The account it matched, to confirm.
    found: Option<Found>,
}

/// The account a user name matched, as the confirm step shows it.
struct Found {
    id: u32,
    login: String,
    /// The character name other players see.
    name: String,
    last_ip: Option<IpAddr>,
    last_sign_in: Option<u64>,
    /// A game of it has a lobby or chat connection open: the reset signs it
    /// out.
    online: bool,
}

impl Web {
    /// Clear the password of `account_id`, logged. `None` for an unknown
    /// account.
    fn reset(&self, account_id: u32, peer: &str) -> Option<PasswordReset> {
        let reset = self.store.reset_password(account_id)?;
        let PasswordReset {
            login,
            sessions_ended,
            written,
        } = &reset;
        match written {
            ResetWrite::Written => log::warn!(
                "Admin from {peer}: reset the password of {login} (account {account_id:#x}); \
                 {sessions_ended} session(s) ended; its next sign-in sets the new one"
            ),
            ResetWrite::Pending => log::warn!(
                "Admin from {peer}: reset the password of {login} (account {account_id:#x}); \
                 {sessions_ended} session(s) ended; the saver writes it (the database was busy)"
            ),
            ResetWrite::Failed(error) => log::error!(
                "Admin from {peer}: reset the password of {login} (account {account_id:#x}) \
                 in memory, {sessions_ended} session(s) ended, but it was not written: \
                 {error}; the saver retries"
            ),
        }
        Some(reset)
    }

    /// The account as the confirm step shows it.
    fn found(&self, account_id: u32) -> Option<Found> {
        let account = self.store.account(account_id)?;
        let (login, name, last_ip, last_sign_in) = {
            let account = account.locked();
            (
                account.login.clone(),
                account.name().to_owned(),
                account.last_ip,
                account.last_sign_in,
            )
        };
        // The name its character has in the game.
        let name = self
            .game
            .accounts
            .get(account_id)
            .map_or(name, |account| account.locked().name.clone());
        Some(Found {
            id: account_id,
            login,
            name,
            last_ip,
            last_sign_in,
            online: self.store.is_online(account_id),
        })
    }

    /// The page's Password tab: the account a user name matched, to
    /// confirm; with the confirm button (`reset_id`, the id it showed) the
    /// reset, if the user name still means that account.
    pub(crate) fn admin_reset_password(
        &self,
        request: &Request,
        peer: &str,
    ) -> (String, ResetView) {
        let lang = request.lang;
        let typed = request.field("reset_user");
        let mut view = ResetView {
            typed: typed.to_owned(),
            found: None,
        };
        let no_such = || {
            lang.t("No account signs in with that user name (character names are not looked up).")
        };
        if typed.is_empty() {
            let message = lang.t("Type the user name the player signs in with.");
            return (message, view);
        }
        // By the user name only, in any case (`persist::user_name_key`, as
        // the `accounts` table's `NOCASE`).
        let Some(account_id) = self.store.account_id_by_name(typed) else {
            return (no_such(), view);
        };
        let confirmed = request.field("reset_id");
        if confirmed.is_empty() {
            view.found = self.found(account_id);
            let message = lang.t("Check the account below, then press the reset button.");
            return (message, view);
        }
        if confirmed.parse::<u32>().ok() != Some(account_id) {
            view.found = self.found(account_id);
            let message = lang.t("That user name now means another account: nothing was changed. Check it below.");
            return (message, view);
        }
        let Some(reset) = self.reset(account_id, peer) else {
            return (no_such(), view);
        };
        let PasswordReset {
            login,
            sessions_ended: ended,
            written,
        } = reset;
        view.typed.clear();
        let message = match written {
            ResetWrite::Written => lang.tf("The password of {login} is cleared; {ended} session(s) ended. Their next sign-in, with any password, sets the new one.", &[("login", &login), ("ended", &ended)]),
            ResetWrite::Pending => lang.tf("The password of {login} is cleared; {ended} session(s) ended. The database was busy: srp's saver writes it within seconds. Their next sign-in, with any password, sets the new one.", &[("login", &login), ("ended", &ended)]),
            ResetWrite::Failed(error) => lang.tf("Error: the password of {login} is cleared and {ended} session(s) ended, but only in memory: the database was not written ({error}). srp retries the write; check its log before the player signs in.", &[("login", &login), ("ended", &ended), ("error", &error)]),
        };
        (message, view)
    }

    /// A script's reset: at once, no confirm step, one line back.
    pub(crate) fn admin_reset_direct(&self, request: &Request, peer: &str) -> Response {
        let answer = |status: &'static str, body: String| Response {
            status,
            content_type: ContentType::Text,
            body: body + "\n",
        };
        match self.check_token(request, peer) {
            Ok(()) => {}
            Err(TokenRefused::Locked) => {
                return answer(
                    "429 Too Many Requests",
                    "locked: too many wrong tokens".into(),
                );
            }
            Err(TokenRefused::Wrong) => return answer("403 Forbidden", "wrong token".into()),
        }
        let typed = request.field("login");
        if typed.is_empty() {
            return answer("400 Bad Request", "no login".into());
        }
        let Some((account_id, reset)) = self
            .store
            .account_id_by_name(typed)
            .and_then(|account_id| Some((account_id, self.reset(account_id, peer)?)))
        else {
            return answer("404 Not Found", "no such login".into());
        };
        match reset.written {
            ResetWrite::Written => answer("200 OK", format!("ok {} {account_id}", reset.login)),
            ResetWrite::Pending => answer(
                "202 Accepted",
                format!("pending {} {account_id}", reset.login),
            ),
            ResetWrite::Failed(error) => answer(
                "500 Internal Server Error",
                format!(
                    "error: {} {account_id} cleared in memory, not written: {error}",
                    reset.login
                ),
            ),
        }
    }
}

/// The Password tab's panel.
pub(crate) fn panel(lang: Lang, view: &super::AdminView) -> String {
    let reset = &view.reset;
    let confirm = match &reset.found {
        Some(found) if !view.token.is_empty() => {
            let last_sign_in = match found.last_sign_in {
                Some(at) => format!("{} {} UTC", date_utc(at), clock_utc(at)),
                None => lang.t("not since srp started"),
            };
            let last_ip = found
                .last_ip
                .map_or_else(|| lang.t("unknown"), |ip| ip.to_string());
            let online = if found.online {
                lang.t("yes: its game is signed out")
            } else {
                lang.t("no")
            };
            format!(
                "<table>\
                 <tr><th>{}</th><td>{}</td></tr>\
                 <tr><th>{}</th><td>{}</td></tr>\
                 <tr><th>{}</th><td>{} ({:#x})</td></tr>\
                 <tr><th>{}</th><td>{}</td></tr>\
                 <tr><th>{}</th><td>{}</td></tr>\
                 <tr><th>{}</th><td>{}</td></tr>\
                 </table>\
                 <button name=reset_id value={}>{}</button>",
                lang.tc("sign-in", "User name"),
                escape(&found.login),
                lang.t("Character"),
                escape(&found.name),
                lang.t("Account id"),
                found.id,
                found.id,
                lang.t("Last sign-in"),
                escape(&last_sign_in),
                lang.t("Last address"),
                escape(&last_ip),
                lang.t("Signed in now"),
                escape(&online),
                found.id,
                escape(&lang.tf("Reset the password of {0}", &[("0", &found.login)])),
            )
        }
        _ => String::new(),
    };
    format!(
        "<div class=\"panel p-password\">\
         <label for=pu>{}</label><input id=pu name=reset_user maxlength=99 value=\"{}\">\
         <small>{}</small><br><button>{}</button>{confirm}</div>",
        lang.t("User name the player signs in with (not the character name)"),
        escape(&reset.typed),
        lang.t("Clears the password: the player's next sign-in, with any password, sets it as the new one (owner's decision, 2026-10-18). Whoever signs in first takes it, so reset when the player is ready to sign in. A game signed in with the account is signed out. Case does not matter."),
        lang.t("Find"),
    )
}

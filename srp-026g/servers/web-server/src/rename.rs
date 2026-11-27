//! Live, free administrator renames by immutable account ID. The panel and
//! release/moderate use this same authenticated action and nickname authority.

use session::v026g::moderation::{AdminAction, clean_reason};
use session::v026g::names::{Nickname, RenameError};

use crate::{Lang, Request, Web, moderation, unix_now};

impl Web {
    pub(crate) fn admin_rename(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let game = &self.game;
        let target = request.field("rename_id").trim();
        if target.is_empty() && request.field("rename_name").is_empty() {
            return String::new();
        }
        let Ok(account_id) = target.strip_prefix('#').unwrap_or(target).parse::<u32>() else {
            return lang.t("Enter an account ID.");
        };
        let Ok(nickname) = Nickname::parse(request.field("rename_name")) else {
            return lang.t("Invalid nickname: at least 3 characters, at most 63 UTF-8 bytes, no spaces or control characters.");
        };
        let Some(reason) = clean_reason(request.field("rename_reason")) else {
            return lang.t("A reason is required.");
        };
        let renamed = match self
            .store
            .admin_rename_026g(&game.accounts, account_id, &nickname)
        {
            Ok(renamed) => renamed,
            Err(RenameError::Taken) => {
                return lang.t("That nickname or a Cyrillic/Latin lookalike is already taken.");
            }
            Err(RenameError::NoAccount) => {
                return lang.t("No such account ID.");
            }
            Err(RenameError::NotEnoughMoney) => unreachable!("administrator renames are free"),
        };
        let action = AdminAction {
            at: unix_now(),
            admin: peer.to_owned(),
            account_id,
            account: nickname.as_str().to_owned(),
            kind: "nickname".to_owned(),
            change: "nickname changed".to_owned(),
            reason,
            before: renamed.previous.clone(),
            after: nickname.as_str().to_owned(),
        };
        log::warn!(
            "Admin from {peer}: renamed account {account_id}: {:?} -> {:?}; reason {:?}",
            action.before,
            action.after,
            action.reason,
        );
        let audit = game.accounts.write_admin(account_id, action);
        game.lobby.account_changed(account_id);
        moderation::log_write(peer, "nickname ownership", &renamed.written);
        moderation::log_write(peer, "nickname audit and game copy", &audit);
        lang.tf("Account #{account_id} renamed: {0} → {1}.", &[("account_id", &account_id), ("0", &renamed.previous), ("1", &nickname.as_str())]) + &moderation::written_note(lang, &renamed.written)
            + &moderation::written_note(lang, &audit)
    }
}

pub(crate) fn panel(lang: Lang) -> String {
    format!(
        "<div class=\"panel p-rename\">\
         <label for=rename-id>{}</label><input id=rename-id name=rename_id inputmode=numeric placeholder=777>\
         <label for=rename-name>{}</label><input id=rename-name name=rename_name maxlength=63>\
         <label for=rename-reason>{}</label><input id=rename-reason name=rename_reason maxlength=300>\
         <p><small>{}</small></p><button>{}</button></div>",
        lang.t("Account ID"),
        lang.t("New nickname"),
        lang.t("Reason (admin history only)"),
        lang.t("Changes the nickname immediately, free of charge. Login and progress stay the same. Duplicate and Cyrillic/Latin lookalike names are refused."),
        lang.t("Rename"),
    )
}

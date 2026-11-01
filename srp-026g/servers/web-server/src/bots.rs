//! The 0.26g admin's Bots tab (A246): the server bots' tuning, as the
//! match server keeps it ([`BotDesk`], srp's `LiveTuning`), shown and
//! changed. A change applies from the next match; a match keeps the tuning
//! it started with.
//!
//! - Each group of knobs is a table: the value the next match runs on (a
//!   field when the group is settable), what a reset gives, the limits.
//! - A field is sent with the value it showed (`was.<key>`), and only the
//!   fields the admin changed are set: two admins saving at once do not put
//!   back each other's changes. The desk refuses the whole when one value
//!   is out of its limits or the values together break the tuning.
//! - "Reset to defaults", for the whole tab or beside a group, first shows
//!   what would go back and waits for its confirm. A reload of the confirm
//!   resets again, which changes nothing more.
//! - The desk audits every change (the log, and the file beside its
//!   changes: when, from which address, the knob, old and new); the tab
//!   shows the latest. The token is in none of them.

use session::v026g::bot_desk::{Change, DeskView, GroupState, Kept, Text};

use super::password_reset::date_utc;
use super::{AdminView, Lang, Request, Web, clock_utc, escape};

/// The Bots tab as shown.
#[derive(Default)]
pub(crate) struct BotsView {
    /// The desk as it stands (after the token only; none on a server
    /// without one).
    desk: Option<DeskView>,
    /// A reset shown, waiting for its confirm.
    confirm: Option<Reset>,
}

/// What a reset puts back to its defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Reset {
    All,
    /// A group's knobs, by its key.
    Group(String),
}

impl Reset {
    fn of(what: &str) -> Self {
        match what {
            "all" => Reset::All,
            key => Reset::Group(key.to_owned()),
        }
    }

    /// Its name in the form.
    fn key(&self) -> &str {
        match self {
            Reset::All => "all",
            Reset::Group(key) => key,
        }
    }

    /// The group the desk resets; `None`: every group.
    fn group(&self) -> Option<&str> {
        match self {
            Reset::All => None,
            Reset::Group(key) => Some(key),
        }
    }
}

/// What a press of the tab asks (`bots_do`). Enter in a field sends the
/// form with whichever button the browser picks, so anything else saves.
enum Action {
    Save,
    /// Show what a reset would put back.
    Ask(Reset),
    Confirm(Reset),
    Cancel,
}

fn action(pressed: &str) -> Action {
    match pressed.split_once(':') {
        Some(("reset", what)) => Action::Ask(Reset::of(what)),
        Some(("confirm", what)) => Action::Confirm(Reset::of(what)),
        _ if pressed == "cancel" => Action::Cancel,
        _ => Action::Save,
    }
}

/// The changes made, as a sentence: `skill.step 0.15 → 0.2, ...`.
fn said(made: &[Change]) -> String {
    made.iter()
        .map(|change| format!("{} {} → {}", change.key, change.old, change.new))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The knobs a reset would put back: those whose value is not their
/// default, in the groups it takes.
fn going_back(view: &DeskView, reset: &Reset) -> Vec<String> {
    view.groups
        .iter()
        .filter(|group| reset.group().is_none_or(|key| key == group.key))
        .flat_map(|group| &group.knobs)
        .filter(|knob| knob.value != knob.default)
        .map(|knob| format!("{} {} → {}", knob.key, knob.value, knob.default))
        .collect()
}

impl Web {
    /// The Bots tab's press: a save, a reset asked or confirmed. The
    /// message says what changed, or why nothing did.
    pub(crate) fn admin_bots(&self, request: &Request, peer: &str) -> (String, BotsView) {
        let lang = request.lang;
        let Some(desk) = self.game.as_ref().and_then(|game| game.bots.clone()) else {
            let message = lang.tr(
                "The bots' tuning is not on this server.",
                "Налаштування ботів на цьому сервері немає.",
            );
            return (message, BotsView::default());
        };
        let refused = |why: String| {
            format!(
                "{} {why}",
                lang.tr("Refused, nothing changed:", "Відмовлено, нічого не змінено:")
            )
        };
        let mut confirm = None;
        let message = match action(request.field("bots_do")) {
            Action::Cancel => String::new(),
            Action::Ask(reset) => {
                let back = going_back(&desk.view(), &reset);
                if back.is_empty() {
                    lang.tr("Nothing to reset: every value is its default.", "Нічого скидати: усі значення типові.")
                } else {
                    confirm = Some(reset);
                    format!(
                        "{} {}",
                        lang.tr(
                            "Confirm below to put these back to their defaults:",
                            "Підтвердьте нижче, щоб повернути типові значення:"
                        ),
                        back.join(", ")
                    )
                }
            }
            Action::Confirm(reset) => match desk.reset(reset.group(), peer) {
                Ok(made) if made.is_empty() => lang.tr("Nothing to reset.", "Нічого скидати."),
                Ok(made) => {
                    log::info!("Admin from {peer}: the bots' tuning reset ({})", reset.key());
                    format!(
                        "{} {}",
                        lang.tr("Reset, from the next match:", "Скинуто, з наступного матчу:"),
                        said(&made)
                    )
                }
                Err(why) => refused(why),
            },
            Action::Save => {
                let asked: Vec<(String, String)> = desk
                    .view()
                    .groups
                    .iter()
                    .filter(|group| group.state == GroupState::Settable)
                    .flat_map(|group| &group.knobs)
                    .filter_map(|knob| {
                        let now = request.field(&format!("bot.{}", knob.key));
                        let was = request.field(&format!("was.{}", knob.key));
                        (!now.is_empty() && now != was).then(|| (knob.key.clone(), now.to_owned()))
                    })
                    .collect();
                if asked.is_empty() {
                    String::new()
                } else {
                    match desk.set(&asked, peer) {
                        Ok(made) if made.is_empty() => {
                            lang.tr("Nothing changed: the values were so already.", "Нічого не змінено: значення вже такі.")
                        }
                        Ok(made) => {
                            log::info!("Admin from {peer}: the bots' tuning changed ({} knobs)", made.len());
                            format!(
                                "{} {}",
                                lang.tr("Saved, from the next match:", "Збережено, з наступного матчу:"),
                                said(&made)
                            )
                        }
                        Err(why) => refused(why),
                    }
                }
            }
        };
        let view = BotsView {
            desk: Some(desk.view()),
            confirm,
        };
        (message, view)
    }
}

/// The Bots tab's panel.
pub(crate) fn bots_panel(lang: Lang, view: &AdminView) -> String {
    let body = match &view.bots.desk {
        _ if view.token.is_empty() => format!(
            "<p><small>{}</small></p>",
            lang.tr(
                "Shown once the token is right.",
                "Показуються після правильного токена."
            )
        ),
        None => format!(
            "<p><small>{}</small></p><button>{}</button>",
            lang.tr(
                "The bots' tuning, as the next match runs it.",
                "Налаштування ботів, як їх візьме наступний матч."
            ),
            lang.tr("Show", "Показати"),
        ),
        Some(desk) => desk_body(lang, desk, view.bots.confirm.as_ref()),
    };
    format!("<div class=\"panel p-bots\">{body}</div>")
}

fn desk_body(lang: Lang, desk: &DeskView, confirm: Option<&Reset>) -> String {
    let text = |text: Text| escape(&lang.tr(text.en, text.uk));
    let kept = match &desk.kept {
        Kept::File(path) => format!(
            "{} {}",
            lang.tr("The changes are kept in", "Зміни зберігаються у"),
            escape(path)
        ),
        Kept::Memory => lang.tr(
            "The changes are kept in memory only: a restart forgets them (SRP_BOT_ADMIN_TUNING).",
            "Зміни лише в пам'яті: перезапуск їх забуде (SRP_BOT_ADMIN_TUNING).",
        ),
    };
    let refused = desk.refused.as_ref().map_or(String::new(), |refused| {
        format!(
            "<p class=note>{} {} {} UTC: {}</p>",
            lang.tr(
                "The matches keep the last good tuning. Refused",
                "Матчі тримають останнє добре налаштування. Відмовлено"
            ),
            date_utc(refused.at),
            clock_utc(refused.at),
            escape(&refused.why)
        )
    });
    let mut groups = String::new();
    for group in &desk.groups {
        let settable = group.state == GroupState::Settable;
        let note = match &group.state {
            GroupState::Settable => String::new(),
            GroupState::Shown(why) => format!("<p><small>{}</small></p>", text(*why)),
        };
        let rows: String = group
            .knobs
            .iter()
            .map(|knob| {
                let value = if settable {
                    let key = escape(&knob.key);
                    let value = escape(&knob.value);
                    format!(
                        "<input name=\"bot.{key}\" value=\"{value}\" inputmode=decimal>\
                         <input type=hidden name=\"was.{key}\" value=\"{value}\">"
                    )
                } else {
                    escape(&knob.value)
                };
                format!(
                    "<tr><td>{}<br><small>{}</small></td><td>{value}</td><td>{}</td><td>{}</td></tr>",
                    text(knob.label),
                    escape(&knob.key),
                    escape(&knob.default),
                    escape(&knob.limits)
                )
            })
            .collect();
        let table = if rows.is_empty() {
            String::new()
        } else {
            format!(
                "<div class=scroll><table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>{rows}</table></div>",
                lang.tr("Knob", "Параметр"),
                lang.tr("Next match", "Наступний матч"),
                lang.tr("Default", "Типове"),
                lang.tr("Limits", "Межі"),
            )
        };
        let reset = if settable {
            format!(
                "<button name=bots_do value=\"reset:{}\" class=quiet>{}</button>",
                escape(group.key),
                lang.tr("Reset this group to defaults", "Скинути цю групу до типових")
            )
        } else {
            String::new()
        };
        groups.push_str(&format!("<h2>{}</h2>{note}{table}{reset}", text(group.title)));
    }
    let confirm = confirm.map_or(String::new(), |reset| {
        format!(
            "<p class=note><button name=bots_do value=\"confirm:{}\">{}</button> \
             <button name=bots_do value=cancel class=quiet>{}</button></p>",
            escape(reset.key()),
            lang.tr("Confirm the reset", "Підтвердити скидання"),
            lang.tr("Cancel", "Скасувати"),
        )
    });
    let audit = if desk.audit.is_empty() {
        format!(
            "<p><small>{}</small></p>",
            lang.tr("No changes yet.", "Змін ще не було.")
        )
    } else {
        let rows: String = desk
            .audit
            .iter()
            .map(|change| {
                format!(
                    "<tr><td>{} {}</td><td>{}</td><td>{}</td><td>{} → {}</td></tr>",
                    date_utc(change.at),
                    clock_utc(change.at),
                    escape(&change.from),
                    escape(&change.key),
                    escape(&change.old),
                    escape(&change.new)
                )
            })
            .collect();
        format!(
            "<div class=scroll><table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>{rows}</table></div>",
            lang.tr("When (UTC)", "Коли (UTC)"),
            lang.tr("From", "Звідки"),
            lang.tr("Knob", "Параметр"),
            lang.tr("Change", "Зміна"),
        )
    };
    format!(
        "<p class=note>{}</p><p><small>{} {}. {kept}</small></p>{refused}{confirm}{groups}\
         <button name=bots_do value=save>{}</button> \
         <button name=bots_do value=\"reset:all\" class=quiet>{}</button>\
         <h2>{}</h2>{audit}",
        lang.tr(
            "Changes apply from the next match; a match keeps the tuning it started with.",
            "Зміни діють з наступного матчу; матч тримає налаштування, з яким почався."
        ),
        lang.tr("The tuning:", "Налаштування:"),
        escape(&desk.source),
        lang.tr("Save", "Зберегти"),
        lang.tr("Reset to defaults", "Скинути до типових"),
        lang.tr("Latest changes", "Останні зміни"),
    )
}

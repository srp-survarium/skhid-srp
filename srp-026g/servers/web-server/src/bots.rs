//! The 0.26g admin's Bots tab (A246): the server bots' tuning, as the
//! match server keeps it ([`BotDesk`], srp's `LiveTuning`), shown and
//! changed. A change applies from the next match; a match keeps the tuning
//! it started with.
//!
//! - Each group of knobs is a table: the value the next match runs on (a
//!   field when the knob is settable), what a reset gives, the limits;
//!   the group's note says what its shown-only knobs wait for.
//! - A field is sent with the value it showed (`was.<key>`), and only the
//!   fields the admin changed are set: two admins saving at once do not put
//!   back each other's changes. The desk refuses the whole when one value
//!   is out of its limits or the values together break the tuning.
//! - [A266] A save first shows what it would change and the expected hit
//!   rates by class, old → new (the desk's preview), with the values typed
//!   kept in their fields; only "Apply" sets them. The rates are the bench's
//!   estimates, and the tab says how they are had and where from.
//! - "Reset to defaults", for the whole tab or beside a group, first shows
//!   what would go back and waits for its confirm. A reload of the confirm
//!   resets again, which changes nothing more.
//! - The desk audits every change (the log, and the file beside its
//!   changes: when, from which address, the knob, old and new); the tab
//!   shows the latest. The token is in none of them.

use session::v026g::bot_desk::{ClassLine, DeskView, Kept, KnobState, Preview, Rates, Text};

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
    /// A save shown, waiting for its apply.
    pending: Option<Pending>,
}

/// [A266] A save shown before it applies: the fields as the admin left them,
/// and what the desk would make of them.
struct Pending {
    fields: Vec<Field>,
    preview: Preview,
}

/// A field the admin changed: its knob, the value typed, the value it showed.
struct Field {
    key: String,
    now: String,
    was: String,
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
/// form with whichever button the browser picks, so anything else shows a
/// save's preview: only "Apply" sets.
enum Action {
    /// Show what the fields changed would make.
    Save,
    Apply,
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
        _ if pressed == "apply" => Action::Apply,
        _ => Action::Save,
    }
}

/// Changes, each `(key, old, new)`, as a sentence: `skill.step 0.15 → 0.2, ...`.
fn said<'a>(made: impl Iterator<Item = (&'a str, &'a str, &'a str)>) -> String {
    made.map(|(key, old, new)| format!("{key} {old} → {new}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The fields the admin changed: each settable knob's that is not what it
/// showed.
fn changed(view: &DeskView, request: &Request) -> Vec<Field> {
    view.groups
        .iter()
        .flat_map(|group| &group.knobs)
        .filter(|knob| knob.state == KnobState::Settable)
        .filter_map(|knob| {
            let now = request.field(&format!("bot.{}", knob.key));
            let was = request.field(&format!("was.{}", knob.key));
            (!now.is_empty() && now != was).then(|| Field {
                key: knob.key.clone(),
                now: now.to_owned(),
                was: was.to_owned(),
            })
        })
        .collect()
}

/// The fields as the desk takes them: each knob and its value typed.
fn asked(fields: &[Field]) -> Vec<(String, String)> {
    fields
        .iter()
        .map(|field| (field.key.clone(), field.now.clone()))
        .collect()
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
        let mut pending = None;
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
                        said(made.iter().map(|c| (c.key.as_str(), c.old.as_str(), c.new.as_str())))
                    )
                }
                Err(why) => refused(why),
            },
            Action::Save => {
                let fields = changed(&desk.view(), request);
                if fields.is_empty() {
                    String::new()
                } else {
                    match desk.preview(&asked(&fields)) {
                        Ok(preview) if preview.edits.is_empty() => {
                            lang.tr("Nothing changed: the values were so already.", "Нічого не змінено: значення вже такі.")
                        }
                        Ok(preview) => {
                            let message = format!(
                                "{} {}",
                                lang.tr(
                                    "Not applied yet: check the change and the expected hit rates, then apply.",
                                    "Ще не застосовано: перевірте зміну й очікувані влучання, потім застосуйте."
                                ),
                                said(preview.edits.iter().map(|e| (e.key.as_str(), e.old.as_str(), e.new.as_str())))
                            );
                            pending = Some(Pending { fields, preview });
                            message
                        }
                        Err(why) => refused(why),
                    }
                }
            }
            Action::Apply => {
                let fields = changed(&desk.view(), request);
                if fields.is_empty() {
                    String::new()
                } else {
                    match desk.set(&asked(&fields), peer) {
                        Ok(made) if made.is_empty() => {
                            lang.tr("Nothing changed: the values were so already.", "Нічого не змінено: значення вже такі.")
                        }
                        Ok(made) => {
                            log::info!("Admin from {peer}: the bots' tuning changed ({} knobs)", made.len());
                            format!(
                                "{} {}",
                                lang.tr("Saved, from the next match:", "Збережено, з наступного матчу:"),
                                said(made.iter().map(|c| (c.key.as_str(), c.old.as_str(), c.new.as_str())))
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
            pending,
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
        Some(desk) => desk_body(lang, desk, view.bots.confirm.as_ref(), view.bots.pending.as_ref()),
    };
    format!("<div class=\"panel p-bots\">{body}</div>")
}

fn desk_body(lang: Lang, desk: &DeskView, confirm: Option<&Reset>, pending: Option<&Pending>) -> String {
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
        let settable = group.knobs.iter().any(|knob| knob.state == KnobState::Settable);
        let note = group.note.map_or(String::new(), |why| format!("<p><small>{}</small></p>", text(why)));
        let source = group.source.as_ref().map_or(String::new(), |source| {
            format!(
                "<p><small>{} {}</small></p>",
                lang.tr("Calibration:", "Калібрування:"),
                escape(source)
            )
        });
        let rows: String = group
            .knobs
            .iter()
            .map(|knob| {
                let value = if knob.state == KnobState::Settable {
                    let key = escape(&knob.key);
                    // A save shown keeps the value typed, and the one it showed.
                    let field = pending.and_then(|pending| pending.fields.iter().find(|field| field.key == knob.key));
                    let (now, was) = field.map_or((knob.value.as_str(), knob.value.as_str()), |field| {
                        (field.now.as_str(), field.was.as_str())
                    });
                    let (now, was) = (escape(now), escape(was));
                    format!(
                        "<input name=\"bot.{key}\" value=\"{now}\" inputmode=decimal>\
                         <input type=hidden name=\"was.{key}\" value=\"{was}\">"
                    )
                } else {
                    escape(&knob.value)
                };
                let beside = knob.beside.as_ref().map_or(String::new(), |beside| {
                    format!("<br><small>{}</small>", escape(beside))
                });
                let value = format!("{value}{beside}");
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
        groups.push_str(&format!("<h2>{}</h2>{note}{source}{table}{reset}", text(group.title)));
    }
    // A save shown: its rates old → new and its apply, first; else the rates
    // as they stand, after the groups.
    let rates = |before: &Rates, after: Option<&Rates>| {
        format!(
            "{}<p><small>{}</small></p><p><small>{} {}</small></p>",
            rates_table(lang, before, after),
            text(before.method),
            lang.tr("Source:", "Джерело:"),
            escape(&before.source)
        )
    };
    let (pending, standing) = match pending {
        Some(pending) => (
            format!(
                "<div class=note><h2>{}</h2>{}<p><button name=bots_do value=apply>{}</button> \
                 <button name=bots_do value=cancel class=quiet>{}</button></p></div>",
                lang.tr("Expected hits a shot, now → with the change", "Очікувані влучання за постріл, зараз → зі зміною"),
                rates(&pending.preview.before, Some(&pending.preview.after)),
                lang.tr("Apply from the next match", "Застосувати з наступного матчу"),
                lang.tr("Cancel", "Скасувати"),
            ),
            String::new(),
        ),
        None => (
            String::new(),
            format!(
                "<h2>{}</h2>{}",
                lang.tr("Expected hits a shot, by class and skill", "Очікувані влучання за постріл, за класом і навичкою"),
                rates(&desk.rates, None)
            ),
        ),
    };
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
        "<p class=note>{}</p><p><small>{} {}. {kept}</small></p>{refused}{confirm}{pending}{groups}{standing}\
         <button name=bots_do value=save>{}</button> \
         <button name=bots_do value=\"reset:all\" class=quiet>{}</button>\
         <h2>{}</h2>{audit}",
        lang.tr(
            "Changes apply from the next match; a match keeps the tuning it started with.",
            "Зміни діють з наступного матчу; матч тримає налаштування, з яким почався."
        ),
        lang.tr("The tuning:", "Налаштування:"),
        escape(&desk.source),
        lang.tr("Save (shows the change first)", "Зберегти (спершу покаже зміну)"),
        lang.tr("Reset to defaults", "Скинути до типових"),
        lang.tr("Latest changes", "Останні зміни"),
    )
}

/// [A266] The expected rates, a class a row and a skill a column (the
/// bands' floor, start and ceiling under their band); with `after`, a value
/// that changes is shown old → new.
fn rates_table(lang: Lang, before: &Rates, after: Option<&Rates>) -> String {
    let text = |text: Text| escape(&lang.tr(text.en, text.uk));
    let mut bands = String::new();
    let mut at = 0;
    while let Some(first) = before.points.get(at) {
        let span = before.points[at..]
            .iter()
            .take_while(|point| point.band == first.band)
            .count();
        bands.push_str(&format!("<th colspan={span}>{}</th>", text(first.band)));
        at += span;
    }
    let skills: String = before
        .points
        .iter()
        .enumerate()
        .map(|(i, point)| {
            let what = point.what.map_or(String::new(), |what| format!("{} ", text(what)));
            let new = after
                .and_then(|after| after.points.get(i))
                .filter(|new| new.skill != point.skill);
            match new {
                Some(new) => format!("<th>{what}<b>{} → {}</b></th>", point.skill, new.skill),
                None => format!("<th>{what}{}</th>", point.skill),
            }
        })
        .collect();
    let values = |line: &ClassLine| -> Vec<Option<String>> {
        match line {
            ClassLine::Own(rates) | ClassLine::Pooled(rates) => {
                rates.iter().map(|rate| Some(format!("{rate:.1}"))).collect()
            }
            ClassLine::Unknown(_) => vec![None; before.points.len()],
        }
    };
    let pooled = format!(
        "<br><small>{}</small>",
        lang.tr(
            "not measured on its own yet: partly the pool's numbers",
            "ще не виміряно окремо: частково спільні числа"
        )
    );
    let rows: String = before
        .classes
        .iter()
        .enumerate()
        .map(|(i, class)| {
            let new = after.and_then(|after| after.classes.get(i)).map(|new| &new.line);
            let cells = match (&class.line, new) {
                (ClassLine::Unknown(why), None | Some(ClassLine::Unknown(_))) => format!(
                    "<td colspan={}><small>{}</small></td>",
                    before.points.len(),
                    text(*why)
                ),
                (old, new) => {
                    let old = values(old);
                    let new = new.map_or_else(|| old.clone(), values);
                    old.iter()
                        .zip(&new)
                        .map(|(old, new)| {
                            let (old, new) = (old.as_deref().unwrap_or("—"), new.as_deref().unwrap_or("—"));
                            if old == new {
                                format!("<td>{old}</td>")
                            } else {
                                format!("<td><b>{old} → {new}</b></td>")
                            }
                        })
                        .collect()
                }
            };
            let mark = match class.line {
                ClassLine::Pooled(_) => pooled.as_str(),
                ClassLine::Own(_) | ClassLine::Unknown(_) => "",
            };
            // The bar of the line shown last (a preview's after), its own words when the line's end is off it.
            let bar = after
                .and_then(|after| after.classes.get(i))
                .unwrap_or(class)
                .bar
                .as_ref()
                .map_or(String::new(), |bar| format!("<br><small>{}</small>", escape(bar)));
            format!("<tr><td>{}{mark}{bar}</td>{cells}</tr>", text(class.label))
        })
        .collect();
    format!(
        "<div class=scroll><table><tr><th rowspan=2>{}</th>{bands}</tr><tr>{skills}</tr>{rows}</table></div>",
        lang.tr("Class", "Клас")
    )
}

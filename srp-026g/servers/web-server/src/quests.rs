//! The wiki's quests, `/wiki/quests` on the 0.26g site: every faction
//! quest, its goal at each reputation level and what it pays. Made from the
//! server's own game data and rules ([`survarium::v026g::quests`], the
//! account's side in [`session::v026g::quests`]), so it is the release's
//! quests and pays what the server pays:
//! - a quest pays silver and reputation, no experience (the data's awards
//!   are those two; [`session::v026g::quests::QuestPay`] has nothing else);
//! - both times the match level's `money_k` and `reputation_k`, the
//!   reputation times `SRP_REPUTATION_RATE` too ([`pay::reputation_rate`],
//!   read as the server reads it), and twice in a match of the daily bonus
//!   ([`bonus::settings`]; [`session::v026g::quests::Scale`]).

use std::collections::BTreeSet;
use std::fmt::Write;

use session::v026g::{bonus, pay, quests::QUESTS_AT_ONCE};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::quests::{FACTIONS, Quest, Quests};

use crate::{Lang, equipment, escape, modifiers};

/// The page's address.
pub(crate) const PATH: &str = "/wiki/quests";

/// A multiplier as the page prints it: `×` and at most two decimals.
fn times(k: f64) -> String {
    let text = format!("{k:.2}");
    format!("×{}", text.trim_end_matches('0').trim_end_matches('.'))
}

/// Distinct values, as one cell: `a`, or `least–most`.
fn span(values: impl IntoIterator<Item = u32>) -> String {
    let values: BTreeSet<u32> = values.into_iter().collect();
    match (values.first(), values.last()) {
        (Some(least), Some(most)) if least != most => format!("{least}–{most}"),
        (Some(value), _) => value.to_string(),
        _ => "—".to_owned(),
    }
}

/// `n` quests, in words.
fn quests_word(lang: Lang, n: usize) -> String {
    let (tens, ones) = (n % 100, n % 10);
    let uk = if (1..=4).contains(&ones) && !(11..=14).contains(&tens) {
        "завдання"
    } else {
        "завдань"
    };
    lang.tr(
        &format!("{n} quest{}", if n == 1 { "" } else { "s" }),
        &format!("{n} {uk}"),
    )
}

/// Minutes as hours, for the intro.
fn hours(minutes: impl IntoIterator<Item = u32>) -> String {
    let values: BTreeSet<u32> = minutes.into_iter().collect();
    let hour = |m: u32| modifiers::number(m as f32 / 60.0);
    match (values.first(), values.last()) {
        (Some(&least), Some(&most)) if least != most => format!("{}–{}", hour(least), hour(most)),
        (Some(&value), _) => hour(value),
        _ => "0".to_owned(),
    }
}

/// A faction's quests by goal, in the data's order: for each goal its
/// quests, which differ by the reputation levels they are offered at.
fn goals(faction: u8) -> Vec<Vec<&'static Quest>> {
    let mut goals: Vec<Vec<&Quest>> = Vec::new();
    for quest in Quests::shared()
        .quests
        .iter()
        .filter(|q| q.faction == faction)
    {
        match goals.iter_mut().find(|goal| goal[0].kind == quest.kind) {
            Some(goal) => goal.push(quest),
            None => goals.push(vec![quest]),
        }
    }
    goals
}

/// The reputation levels `quest` is offered at, up to the faction's last.
fn levels(quest: &Quest, last: u8) -> std::ops::RangeInclusive<u8> {
    quest.min_level..=quest.max_level.min(last)
}

/// One faction's table: a row a goal, a column a reputation level, the
/// target in the cell.
fn faction_table(lang: Lang, out: &mut String, faction: u8) {
    let last = Dictionary::shared()
        .factions
        .iter()
        .find(|row| row.id == faction)
        .map_or(u8::MAX, |row| row.levels.len().saturating_sub(1) as u8);
    let goals = goals(faction);
    let columns: BTreeSet<u8> = goals
        .iter()
        .flatten()
        .flat_map(|quest| levels(quest, last))
        .collect();
    let _ = write!(
        out,
        "<section class=quest-faction id=faction-{faction}><h2>{}</h2>\
         <div class=table-scroll><table class=\"protection quests\"><thead>\
         <tr><th rowspan=2>{}</th><th rowspan=2>{}</th><th colspan={}>{}</th><th rowspan=2>{}</th><th rowspan=2>{}</th></tr><tr>",
        escape(&equipment::faction_name(lang, faction)),
        lang.tr("Quest", "Завдання"),
        lang.tr("Goal", "Мета"),
        columns.len(),
        lang.tr("At reputation level", "На рівні репутації"),
        escape(&modifiers::text(lang, "st_regular_money")),
        escape(&modifiers::text(lang, "st_reputation")),
    );
    for level in &columns {
        let _ = write!(out, "<th>{level}</th>");
    }
    out.push_str("</tr></thead><tbody>");
    for goal in &goals {
        let first = goal[0];
        let objective = modifiers::text(lang, &first.objective);
        let _ = write!(
            out,
            "<tr><th>{}</th><td class=quest-goal>{}</td>",
            escape(&modifiers::text(lang, &first.name)),
            escape(objective.trim_end_matches(':')),
        );
        for &level in &columns {
            let targets: Vec<&str> = goal
                .iter()
                .filter(|quest| levels(quest, last).contains(&level))
                .map(|quest| quest.target.as_deref().unwrap_or("✓"))
                .collect();
            let cell = if targets.is_empty() {
                "—".to_owned()
            } else {
                targets.join(" / ")
            };
            let _ = write!(out, "<td>{}</td>", escape(&cell));
        }
        let reputation = goal.iter().flat_map(|quest| {
            quest
                .reputation
                .iter()
                .filter(|&&(to, _)| to == faction)
                .map(|&(_, amount)| amount)
        });
        let _ = write!(
            out,
            "<td>{}</td><td>{}</td></tr>",
            span(goal.iter().map(|quest| quest.money)),
            span(reputation),
        );
    }
    out.push_str("</tbody></table></div></section>");
}

/// What a match level makes of the tables' silver and reputation.
fn level_table(lang: Lang, out: &mut String) {
    let rate = f64::from(pay::reputation_rate()) / 100.0;
    let levels = &Dictionary::shared().match_levels;
    let _ = write!(
        out,
        "<div class=table-scroll><table class=\"protection quests quest-levels\"><thead><tr><th>{}</th>",
        escape(&modifiers::text(lang, "st_match_making_level")),
    );
    for level in 0..levels.len() {
        let _ = write!(out, "<th>{level}</th>");
    }
    out.push_str("</tr></thead><tbody>");
    let rows = [
        (
            "st_regular_money",
            levels
                .iter()
                .map(|level| pay::multiplier(level.money_k))
                .collect::<Vec<_>>(),
        ),
        (
            "st_reputation",
            levels
                .iter()
                .map(|level| pay::multiplier(level.reputation_k) * rate)
                .collect(),
        ),
    ];
    for (name, ks) in rows {
        let _ = write!(out, "<tr><th>{}</th>", escape(&modifiers::text(lang, name)));
        for k in ks {
            let _ = write!(out, "<td>{}</td>", times(k));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
}

/// The page's body in `lang`, for the wiki's page.
pub(crate) fn body(lang: Lang) -> String {
    let quests = &Quests::shared().quests;
    let code = lang.code();
    let lasts = hours(quests.iter().map(|q| q.ttl_minutes));
    let rests = hours(quests.iter().map(|q| q.cooldown_minutes));
    let rate = times(f64::from(pay::reputation_rate()) / 100.0);
    let wins = bonus::settings().enabled.then_some(bonus::WINS_A_DAY);
    let (no_experience, intro, scaled, doubled, uncounted) = match lang {
        Lang::En => (
            "Quests give no experience: they pay silver and reputation.".to_owned(),
            format!(
                "Choose a faction in the lobby: it gives you {QUESTS_AT_ONCE} of its quests at a time, \
                 for your reputation level with it. A quest lasts {lasts} h; one you complete comes back after {rests} h."
            ),
            format!(
                "The silver and reputation above are multiplied by the level of the match the quest is \
                 completed in; reputation includes Skhid's {rate}."
            ),
            wins.map(|wins| format!(" Reputation counts twice in the day's first {wins} wins.")),
            "Matches against bots and rooms with a PIN count for no quest. Grenade quests come once a shop \
             sells you RGD-5, artefact quests once one sells you gear with an artefact container. \
             Battery quests count only in battery modes.",
        ),
        Lang::Uk => (
            "Завдання не дають досвіду: за них платять срібло й репутацію.".to_owned(),
            format!(
                "Оберіть угруповання в лобі: воно дає {} водночас, за вашим рівнем репутації з ним. \
                 Завдання триває {lasts} год; виконане повертається за {rests} год.",
                quests_word(lang, QUESTS_AT_ONCE),
            ),
            format!(
                "Срібло й репутацію з таблиць вище множать на рівень матчу, у якому завдання виконано; \
                 у репутації вже є множник Сходу {rate}."
            ),
            wins.map(|wins| format!(" У перших {wins} перемогах за день репутація подвоюється.")),
            "Бої проти ботів і кімнати з PIN не зараховуються до завдань. Завдання на гранати з'являються, \
             коли крамниця продає вам РГД-5, на артефакти — коли продає спорядження з контейнером для артефактів. \
             Завдання на батареї зараховуються лише в режимах з батареями.",
        ),
    };
    let mut out = format!(
        "<p class=breadcrumb><a href=\"/wiki?lang={code}\">{}</a> / {}</p>\
         <div class=wiki-heading><div><h1>{}</h1></div><span class=catalogue-count>{}</span></div>\
         <p class=wiki-intro><strong>{}</strong> {}</p>",
        lang.tr("Wiki", "Вікі"),
        lang.tr("Quests", "Завдання"),
        lang.tr("Quests", "Завдання"),
        quests_word(lang, quests.len()),
        escape(&no_experience),
        escape(&intro),
    );
    for faction in FACTIONS {
        faction_table(lang, &mut out, faction);
    }
    let _ = write!(
        out,
        "<section class=quest-faction id=match-level><h2>{}</h2><p class=wiki-note>{}{}</p>",
        lang.tr("Pay by match level", "Оплата за рівнем матчу"),
        escape(&scaled),
        escape(&doubled.unwrap_or_default()),
    );
    level_table(lang, &mut out);
    let _ = write!(
        out,
        "<p class=wiki-note>{}</p></section>",
        escape(uncounted)
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_faction_quest_is_on_the_page_with_its_pay() {
        let quests = &Quests::shared().quests;
        for lang in [Lang::En, Lang::Uk] {
            let page = crate::wiki::page(PATH, "", lang, "site header").body;
            assert!(page.contains("site header"));
            assert!(page.contains(&format!("lang=\"{}\"", lang.code())));
            assert!(page.contains(&quests_word(lang, quests.len())));
            for faction in FACTIONS {
                assert!(
                    page.contains(&format!("id=faction-{faction}>")),
                    "{faction}"
                );
            }
            // A row a goal of each faction, and every quest's target in one.
            let rows: usize = FACTIONS.iter().map(|&faction| goals(faction).len()).sum();
            assert_eq!(page.matches("<td class=quest-goal>").count(), rows);
            assert_eq!(
                FACTIONS.into_iter().flat_map(goals).flatten().count(),
                quests.len()
            );
            // The pay as the server pays it: no experience, no gold, the
            // reputation at its rate.
            assert!(!page.to_lowercase().contains("gold") && !page.contains("золот"));
            let rate = times(f64::from(pay::reputation_rate()) / 100.0);
            assert!(page.contains(&format!(
                "<th>{}</th><td>{rate}</td>",
                modifiers::text(lang, "st_reputation")
            )));
        }
        let en = body(Lang::En);
        assert!(en.contains("Quests give no experience"));
        // Scavengers' first: defend the batteries, a goal without a number, 50 and 50.
        assert!(en.contains("<tr><th>Possessions defence</th><td class=quest-goal>Win a match without losing batteries</td><td>✓</td>"));
        assert!(en.contains("<td>50</td><td>50</td></tr>"));
    }

    #[test]
    fn a_multiplier_and_a_span_print_short() {
        assert_eq!(times(1.0), "×1");
        assert_eq!(times(1.2), "×1.2");
        assert_eq!(times(13.59), "×13.59");
        assert_eq!(span([75, 50, 75]), "50–75");
        assert_eq!(span([150]), "150");
        assert_eq!(hours([1440]), "24");
    }
}

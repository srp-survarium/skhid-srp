//! The wiki's quests, `/wiki/quests` on the 0.26g site: every faction
//! quest, its goal at each reputation level and what it pays. Made from the
//! server's own game data and rules ([`survarium::v026g::quests`], the
//! account's side in [`session::v026g::quests`]), so it is the release's
//! quests and pays what the server pays:
//! - a quest pays silver and reputation, no experience (the data's awards
//!   are those two; [`session::v026g::quests::QuestPay`] has nothing else);
//! - both times the match level's `money_k` and `reputation_k`, the
//!   reputation times `SRP_REPUTATION_RATE` too ([`pay::reputation_rate`],
//!   read as the server reads it; [`session::v026g::quests::Scale`]);
//! - the match levels shown stop at the highest a match can have
//!   ([`matchmaking::highest_match_level`]).
//!
//! Laid out as the modifiers page is: a section a faction (its emblem, the
//! quest window's), a row a goal (its quest-list icon, name and pay, the goal,
//! and its targets by reputation level as short wrapping runs).

use std::collections::BTreeSet;
use std::fmt::Write;

use session::v026g::{matchmaking, pay};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::quests::{FACTIONS, Quest, Quests};

use crate::icons::{self, IconSet};
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

/// A goal's targets by reputation level: each run of levels with one target,
/// as the lobby prints it after the goal ("0–2: 5"), or the levels alone for
/// a goal without a number. A level's target is its first quest's.
fn targets(goal: &[&Quest], last: u8) -> Vec<String> {
    let mut runs: Vec<(u8, u8, Option<&str>)> = Vec::new();
    for level in 0..=last {
        let Some(quest) = goal
            .iter()
            .find(|quest| levels(quest, last).contains(&level))
        else {
            continue;
        };
        let target = quest.target.as_deref();
        match runs.last_mut() {
            Some((_, end, run)) if *end + 1 == level && *run == target => *end = level,
            _ => runs.push((level, level, target)),
        }
    }
    runs.into_iter()
        .map(|(first, end, target)| {
            let span = if first == end {
                first.to_string()
            } else {
                format!("{first}–{end}")
            };
            target.map_or_else(|| span.clone(), |target| format!("{span}: {target}"))
        })
        .collect()
}

/// One faction's section: a row a goal.
fn faction_section(lang: Lang, out: &mut String, faction: u8) {
    let last = Dictionary::shared()
        .factions
        .iter()
        .find(|row| row.id == faction)
        .map_or(u8::MAX, |row| row.levels.len().saturating_sub(1) as u8);
    let emblem = icons::url(IconSet::Faction, &faction.to_string())
        .map_or_else(String::new, |url| {
            format!("<img class=quest-emblem src=\"{}\" alt=\"\">", escape(&url))
        });
    let _ = write!(
        out,
        "<section class=\"modifiers quests\" id=faction-{faction}><h2>{emblem}{}</h2>",
        escape(&equipment::faction_name(lang, faction)),
    );
    let silver = modifiers::text(lang, "st_regular_money");
    let reputation = modifiers::text(lang, "st_reputation");
    for goal in goals(faction) {
        let first = goal[0];
        let objective = modifiers::text(lang, &first.objective);
        let paid = goal.iter().flat_map(|quest| {
            quest
                .reputation
                .iter()
                .filter(|&&(to, _)| to == faction)
                .map(|&(_, amount)| amount)
        });
        let runs: String = targets(&goal, last)
            .iter()
            .map(|run| format!("<li>{}</li>", escape(run)))
            .collect();
        let _ = write!(
            out,
            "<div class=\"modifier quest\"><div><h3>{}{}</h3><p class=quest-pay>{} {} · {} {}</p></div>\
             <p class=quest-goal>{}</p><ol class=strengths>{runs}</ol></div>",
            modifiers::glyph(IconSet::Quest, &first.icon.to_string()),
            escape(&modifiers::text(lang, &first.name)),
            escape(&silver),
            span(goal.iter().map(|quest| quest.money)),
            escape(&reputation),
            span(paid),
            escape(objective.trim_end_matches(':')),
        );
    }
    out.push_str("</section>");
}

/// What each match level a match can have makes of the silver and
/// reputation: a cell a level, wrapping.
fn level_grid(lang: Lang, out: &mut String) {
    let rate = f64::from(pay::reputation_rate()) / 100.0;
    let highest = usize::from(matchmaking::highest_match_level());
    let _ = write!(
        out,
        "<ol class=quest-levels><li class=quest-levels-key><b>{}</b><span>{}</span><span>{}</span></li>",
        escape(&modifiers::text(lang, "st_match_making_level")),
        escape(&modifiers::text(lang, "st_regular_money")),
        escape(&modifiers::text(lang, "st_reputation")),
    );
    for (level, row) in Dictionary::shared()
        .match_levels
        .iter()
        .enumerate()
        .take(highest + 1)
    {
        let _ = write!(
            out,
            "<li><b>{level}</b><span>{}</span><span>{}</span></li>",
            times(pay::multiplier(row.money_k)),
            times(pay::multiplier(row.reputation_k) * rate),
        );
    }
    out.push_str("</ol>");
}

/// The page's body in `lang`, for the wiki's page.
pub(crate) fn body(lang: Lang) -> String {
    let code = lang.code();
    let mut out = format!(
        "<p class=breadcrumb><a href=\"/wiki?lang={code}\">{}</a> / {}</p>\
         <div class=wiki-heading><div><h1>{}</h1></div></div>",
        lang.tr("Wiki", "Вікі"),
        lang.tr("Quests", "Завдання"),
        lang.tr("Quests", "Завдання"),
    );
    for faction in FACTIONS {
        faction_section(lang, &mut out, faction);
    }
    let _ = write!(
        out,
        "<section class=\"modifiers quests\" id=match-level><h2>{}</h2>",
        lang.tr("Pay by match level", "Оплата за рівнем матчу"),
    );
    level_grid(lang, &mut out);
    out.push_str("</section>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_faction_quest_is_on_the_page_with_its_icons_and_pay() {
        let quests = &Quests::shared().quests;
        for lang in [Lang::En, Lang::Uk] {
            let page = crate::wiki::page(PATH, "", lang, "site header", None).body;
            assert!(page.contains("site header"));
            assert!(page.contains(&format!("lang=\"{}\"", lang.code())));
            for faction in FACTIONS {
                assert!(
                    page.contains(&format!("id=faction-{faction}>")),
                    "{faction}"
                );
            }
            // A row a goal of each faction, every quest in one.
            let rows: usize = FACTIONS.iter().map(|&faction| goals(faction).len()).sum();
            assert_eq!(page.matches("<div class=\"modifier quest\">").count(), rows);
            assert_eq!(
                FACTIONS.into_iter().flat_map(goals).flatten().count(),
                quests.len()
            );
            // No experience, no gold; the reputation at its rate; the match
            // levels up to the highest a match can have.
            assert!(!page.to_lowercase().contains("gold") && !page.contains("золот"));
            let rate = times(f64::from(pay::reputation_rate()) / 100.0);
            assert!(page.contains(&format!(
                "<li><b>0</b><span>×1</span><span>{rate}</span></li>"
            )));
            let highest = matchmaking::highest_match_level();
            assert!(page.contains(&format!("<li><b>{highest}</b>")));
            assert!(!page.contains(&format!("<li><b>{}</b>", highest + 1)));
        }
        let en = body(Lang::En);
        // Scavengers' first: defend the batteries, levels 0 to 5, no number, 50 and 50.
        assert!(en.contains(
            "Possessions defence</h3><p class=quest-pay>Silver 50 · Reputation 50</p></div>\
             <p class=quest-goal>Win a match without losing batteries</p><ol class=strengths><li>0–5</li></ol>"
        ));
        // Their kill/death ratio: a run a level.
        assert!(en.contains("<li>0: 1</li><li>1: 1.1</li>"));
    }

    #[test]
    fn every_icon_the_page_names_is_served() {
        for quest in &Quests::shared().quests {
            assert!(
                icons::url(IconSet::Quest, &quest.icon.to_string()).is_some(),
                "quest {}",
                quest.id
            );
        }
        let page = body(Lang::En);
        let files: Vec<&str> = page
            .match_indices("/wiki/icons/")
            .map(|(at, _)| page[at..].split_once("?v=").unwrap().0)
            .collect();
        assert!(
            files
                .iter()
                .any(|file| file.starts_with("/wiki/icons/quests/"))
        );
        assert_eq!(
            files
                .iter()
                .filter(|file| file.starts_with("/wiki/icons/factions/"))
                .count(),
            FACTIONS.len()
        );
        for file in files {
            let (content_type, bytes) = icons::file(file).unwrap_or_else(|| panic!("{file}"));
            assert_eq!(content_type, "image/webp", "{file}");
            assert!(!bytes.is_empty(), "{file}");
        }
    }

    #[test]
    fn targets_are_runs_of_levels() {
        let goal: Vec<&Quest> = goals(1)
            .into_iter()
            .find(|goal| goal[0].kind == 12)
            .unwrap();
        // Scavengers' kills: 5, 6, 7, 8, 9, 9 at levels 0 to 5.
        assert_eq!(
            targets(&goal, 5),
            ["0: 5", "1: 6", "2: 7", "3: 8", "4–5: 9"]
        );
    }

    #[test]
    fn a_multiplier_and_a_span_print_short() {
        assert_eq!(times(1.0), "×1");
        assert_eq!(times(1.2), "×1.2");
        assert_eq!(times(13.59), "×13.59");
        assert_eq!(span([75, 50, 75]), "50–75");
        assert_eq!(span([150]), "150");
    }
}

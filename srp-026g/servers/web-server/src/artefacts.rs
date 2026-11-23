//! The wiki's artefacts, `/wiki/artefacts` on the 0.26g site: every artefact of the game data
//! with what it gives carried (its passive) and fired (its active), its active time, cooldown and
//! weight, its skill of the artefact tree, the rules of carrying several (docs/notes/artefacts.md,
//! exe patches 54 and 109), the items whose built-in bonuses change them, the maps whose anomalies
//! spawn it and Keeper of the Artifacts' share of it. Made from the server's own game data
//! ([`survarium::v026g::artefacts`], the maps' anomalies, the skills tree), so it is the
//! release's. The names are the game's: its pickup hint names each artefact.

use std::collections::BTreeMap;
use std::fmt::Write;

use srp_validate::world::Artefact as Kind;
use survarium::v026g::armour::BodyPart;
use survarium::v026g::artefacts::{
    self, Affine, ArtefactConfig, Effects, Modifier, Pool, Reduction,
};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::keeper;
use survarium::v026g::localization;
use survarium::v026g::maps::{self, GameMode};
use survarium::v026g::skills::SkillsTree;

use crate::icons::IconSet;
use crate::modifiers::{self, number};
use crate::{Lang, equipment, escape, maps as maps_page, skills};

/// The page's address.
pub(crate) const PATH: &str = "/wiki/artefacts";

/// Each kind's skill of the artefact tree (exe patch 109: Spring II, Onyx II, Larkspur II).
fn own_skill(kind: Kind) -> u16 {
    match kind {
        Kind::Spring => 531,
        Kind::Onyx => 532,
        Kind::Lifebone => 533,
    }
}

/// The artefact tree's other skills that change how artefacts are found, carried or fired
/// (docs/notes/artefact-skills.md): Quick Hands, Knowledge of Artifacts, Artifact Sight,
/// Keeper of the Artifacts, Artifact Container, Courier, Resonance, Cascade.
const SKILLS: [u16; 8] = [501, 512, 513, 521, 522, 534, 541, 542];

/// Keeper of the Artifacts.
const KEEPER: u16 = 521;

/// An artefact's name in `lang`: its pickup hint's first sentence without the language's word
/// for an artefact ("Onyx artifact.", "Артефакт Оникс."); a language without the text has the
/// English one. The dictionary's name if neither says it.
fn name(lang: Lang, artefact: &ArtefactConfig) -> String {
    let said = |lang: Lang| {
        let hint = localization::text(modifiers::language(lang), &artefact.pickup_hint)?;
        let first = hint.trim().split('.').next()?.trim();
        let name = match lang {
            Lang::En => first.strip_suffix(" artifact"),
            Lang::Hu => first.strip_suffix(" artefaktum"),
            Lang::Fi => first.strip_suffix("-artefakti"),
            Lang::Uk | Lang::Ru => first.strip_prefix("Артефакт "),
            Lang::Be => first.strip_prefix("Артэфакт "),
            Lang::Pt => first.strip_prefix("Artefato "),
            Lang::Pl | Lang::Sr | Lang::Cs | Lang::Hr => first.strip_prefix("Artefakt "),
            Lang::De => {
                let rest = first.strip_prefix("Das ").unwrap_or(first);
                rest.strip_prefix("Artefakt ")
                    .map(|rest| rest.strip_prefix("der ").unwrap_or(rest))
            }
        }?;
        Some(name.trim().to_owned()).filter(|name| !name.is_empty())
    };
    said(lang).or_else(|| said(Lang::En)).unwrap_or_else(|| {
        Dictionary::shared()
            .item(artefact.dict_id)
            .map_or_else(String::new, |item| item.name.clone())
    })
}

/// A signed percentage of `fraction`.
fn percent(fraction: f32) -> String {
    let sign = if fraction > 0.0 { "+" } else { "" };
    format!("{sign}{}%", number(fraction * 100.0))
}

/// A player modifier's change: a percentage, or the factor `max(0, 1 + v)` once it is no
/// share (stamina regeneration ×6, no stamina use ×0).
fn modifier(Modifier(value): Modifier) -> String {
    if value.abs() < 1.0 {
        percent(value)
    } else {
        format!("×{}", number((1.0 + value).max(0.0)))
    }
}

/// `(x + add) × mul` of health: its terms that change something.
fn affine(lang: Lang, Affine { add, mul }: Affine) -> String {
    let mut terms = Vec::new();
    if add != 0.0 {
        let sign = if add > 0.0 { "+" } else { "" };
        terms.push(format!(
            "{sign}{} {}",
            number(add * 100.0),
            lang.tc("unit", "HP")
        ));
    }
    if mul != 1.0 {
        terms.push(percent(mul - 1.0));
    }
    terms.join(" ")
}

fn pool_name(lang: Lang, pool: Pool) -> String {
    match pool {
        Pool::Body => lang.t("Body"),
        Pool::Head => lang.t("Head"),
        Pool::Pain => lang.t("Pain"),
        Pool::LeftHand | Pool::RightHand => lang.t("Hands"),
        Pool::LeftLeg | Pool::RightLeg => lang.t("Legs"),
    }
}

/// Names of `pools`, each once, both sides as one.
fn pool_names(lang: Lang, pools: &[Pool]) -> String {
    let mut names: Vec<String> = Vec::new();
    for &pool in pools {
        let name = pool_name(lang, pool);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names.join(", ")
}

/// A line of an effect: its name and value.
type Line = (String, String);

fn onyx_lines(lang: Lang, reduction: Reduction) -> Vec<Line> {
    vec![
        (lang.t("Damage"), affine(lang, reduction.damage)),
        (lang.t("Penetration"), affine(lang, reduction.piercing)),
    ]
}

/// What a kind gives carried, and fired (on top of it).
fn lines(lang: Lang, effects: &Effects) -> (Vec<Line>, Vec<Line>) {
    match effects {
        Effects::Spring {
            speed,
            aim_time,
            active,
        } => {
            let mut passive = vec![(lang.t("Movement speed"), modifier(*speed))];
            passive.extend(aim_time.map(|aim| (lang.t("Aim time"), modifier(aim))));
            let mut fired = vec![(lang.t("Movement speed"), modifier(active.speed))];
            fired.extend(
                active
                    .aim_time
                    .map(|aim| (lang.t("Aim time"), modifier(aim))),
            );
            fired.push((
                lang.t("Stamina regeneration"),
                modifier(active.stamina_regeneration),
            ));
            fired.push((lang.t("Stamina use"), modifier(active.stamina_use)));
            fired.extend(
                active
                    .stopping_power
                    .map(|stop| (lang.t("Slowdown from hits"), modifier(stop))),
            );
            (passive, fired)
        }
        Effects::Onyx {
            hit,
            parts,
            passive,
            active,
        } => {
            // Only parts it leaves out are worth a line.
            let mut whom = vec![(lang.t("Against"), equipment::hit_name(lang, *hit))];
            if BodyPart::ALL.iter().any(|part| !parts.contains(part)) {
                let mut names: Vec<String> = Vec::new();
                for &part in parts {
                    let name = equipment::part_name(lang, part);
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
                whom.push((lang.t("Body parts"), names.join(", ")));
            }
            let mut carried = whom.clone();
            carried.extend(onyx_lines(lang, *passive));
            let mut fired = whom;
            fired.extend(onyx_lines(lang, *active));
            (carried, fired)
        }
        Effects::Lifebone {
            regeneration,
            cures,
        } => {
            // The pools with the same terms together.
            let mut groups: Vec<(Vec<Pool>, (Affine, Affine, Affine))> = Vec::new();
            for row in regeneration {
                let terms = (row.speed, row.delay, row.limit);
                match groups.iter_mut().find(|(_, same)| *same == terms) {
                    Some((pools, _)) => pools.push(row.pool),
                    None => groups.push((vec![row.pool], terms)),
                }
            }
            let one = groups.len() == 1;
            let mut carried = Vec::new();
            for (pools, (speed, delay, limit)) in groups {
                let of = |label: String| {
                    if one {
                        label
                    } else {
                        format!("{label} ({})", pool_names(lang, &pools))
                    }
                };
                if speed.add != 0.0 || speed.mul != 1.0 {
                    let mut terms = Vec::new();
                    if speed.add != 0.0 {
                        let sign = if speed.add > 0.0 { "+" } else { "" };
                        let per_second = lang.tc("unit", "HP/s");
                        terms.push(format!("{sign}{} {per_second}", number(speed.add * 100.0)));
                    }
                    if speed.mul != 1.0 {
                        terms.push(percent(speed.mul - 1.0));
                    }
                    carried.push((of(lang.t("Regeneration")), terms.join(" ")));
                }
                if delay.add != 0.0 || delay.mul != 1.0 {
                    let mut terms = Vec::new();
                    if delay.add != 0.0 {
                        let sign = if delay.add > 0.0 { "+" } else { "" };
                        terms.push(format!(
                            "{sign}{} {}",
                            number(delay.add),
                            lang.tc("unit", "s")
                        ));
                    }
                    if delay.mul != 1.0 {
                        terms.push(percent(delay.mul - 1.0));
                    }
                    carried.push((of(lang.t("Regeneration delay")), terms.join(" ")));
                }
                // The limit is a share of the pool's maximum, at most all of it.
                if limit.add >= 1.0 && limit.mul >= 1.0 {
                    carried.push((of(lang.t("Regenerates up to")), "100%".to_owned()));
                } else if limit.add != 0.0 || limit.mul != 1.0 {
                    let mut terms = Vec::new();
                    if limit.add != 0.0 {
                        terms.push(percent(limit.add));
                    }
                    if limit.mul != 1.0 {
                        terms.push(format!("×{}", number(limit.mul)));
                    }
                    carried.push((of(lang.t("Regenerates up to")), terms.join(" ")));
                }
            }
            // Exe patch 54: the active fills every pool to its maximum.
            let mut fired = vec![(lang.t("Health"), "100%".to_owned())];
            if !cures.is_empty() {
                fired.push((lang.t("Cures injuries"), pool_names(lang, cures)));
            }
            (carried, fired)
        }
    }
}

fn list(lines: &[Line]) -> String {
    let items: String = lines
        .iter()
        .map(|(label, value)| format!("<li>{} <b>{}</b></li>", escape(label), escape(value)))
        .collect();
    format!("<ul>{items}</ul>")
}

/// A file of the skills page with its version, as its own pages name it.
fn skill_icon(id: u16) -> Option<String> {
    let path = format!("/skills/icons/{id}.webp");
    let (_, bytes) = skills::file(&path)?;
    let version: String = openssl::sha::sha256(bytes)[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Some(format!("{path}?v={version}"))
}

fn skill_link(lang: Lang, id: u16) -> Option<String> {
    let skill = SkillsTree::shared().skill(id)?;
    let icon = skill_icon(id).map_or_else(String::new, |url| {
        format!(
            "<img src=\"{}\" alt=\"\" width=28 height=28 loading=lazy>",
            escape(&url)
        )
    });
    Some(format!(
        "<a href=\"{}?lang={}\">{icon}<span>{}</span></a>",
        skills::PATH,
        lang.code(),
        escape(&modifiers::text(lang, &skill.name))
    ))
}

/// The maps whose anomalies spawn each kind, by the Maps tab's key (the project's folder):
/// its playable maps (Blackout's configs aside, as the tab), an anomaly with containers.
fn spawns() -> BTreeMap<(maps::Build, &'static str), (&'static str, Vec<Kind>)> {
    let mut found: BTreeMap<(maps::Build, &str), (&str, Vec<Kind>)> = BTreeMap::new();
    for map in maps::playable().filter(|map| map.mode != GameMode::Blackout) {
        let folder = map.project.split('/').next().unwrap_or(map.project);
        for anomaly in map.world_objects.anomalies {
            let Some(artefacts) = &anomaly.artefacts else {
                continue;
            };
            if artefacts.containers == 0 {
                continue;
            }
            let (_, kinds) = found
                .entry((map.build, folder))
                .or_insert((map.map_name_key, Vec::new()));
            for item in artefacts.artefacts {
                if !kinds.contains(&item.core) {
                    kinds.push(item.core);
                }
            }
        }
    }
    found
}

/// The items with built-in bonuses to artefacts (the Artifact-master's cooldown and search
/// time): a line each.
fn item_lines(lang: Lang) -> Vec<String> {
    Dictionary::shared()
        .items
        .iter()
        .filter_map(|item| {
            let bonuses: Vec<String> = modifiers::built_in(item.dict_id)
                .into_iter()
                .filter(|(modification, _)| {
                    modification
                        .modifiers
                        .iter()
                        .any(|modifier| modifier.path.starts_with("parameters/artefact_"))
                })
                .map(|(modification, property)| {
                    format!(
                        "{} {}",
                        modifiers::property_name(lang, property),
                        modifiers::strength(lang, property, modification.shown.value)
                    )
                })
                .collect();
            (!bonuses.is_empty()).then(|| {
                format!(
                    "{}: {}",
                    modifiers::item_name(lang, item.dict_id),
                    bonuses.join(", ")
                )
            })
        })
        .collect()
}

/// The page's body in `lang`.
pub(crate) fn body(lang: Lang) -> String {
    let code = lang.code();
    let seconds = modifiers::text(lang, "st_item_props_sec");
    let kg = modifiers::text(lang, "st_item_props_kg");
    let all = artefacts::all();

    let mut rows = String::new();
    for artefact in all {
        let (carried, fired) = lines(lang, &artefact.effects);
        let picture = crate::icons::url(
            IconSet::Item,
            &Dictionary::shared()
                .item(artefact.dict_id)
                .map_or_else(String::new, |item| item.name.clone()),
        )
        .map_or_else(String::new, |url| {
            format!(
                "<img src=\"{}\" alt=\"\" width=86 height=42 loading=lazy>",
                escape(&url)
            )
        });
        let skill = SkillsTree::shared()
            .skill(own_skill(artefact.kind))
            .and_then(|skill| {
                Some(format!(
                    "{}<p>{}</p>",
                    skill_link(lang, skill.id)?,
                    escape(&modifiers::text(lang, &skill.description))
                ))
            })
            .unwrap_or_default();
        let _ = write!(
            rows,
            "<tr id=\"{}\"><th>{picture}<span>{}</span></th><td>{}</td><td>{}</td>\
             <td class=num>{} {seconds}</td><td class=num>{} {seconds}</td>\
             <td class=num>{} {kg}</td><td class=artefact-skill>{skill}</td></tr>",
            anchor(artefact.kind),
            escape(&name(lang, artefact)),
            list(&carried),
            list(&fired),
            number(artefact.active_s),
            number(artefact.cooldown_s),
            number(artefact.weight),
        );
    }
    let table = format!(
        "<div class=table-scroll><table class=artefact-table><thead><tr><th>{}</th><th>{}</th>\
         <th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr></thead><tbody>{rows}\
         </tbody></table></div>",
        lang.t("Artefact"),
        lang.t("Passive"),
        lang.t("Active"),
        lang.t("Active time"),
        lang.t("Cooldown"),
        lang.t("Weight"),
        lang.t("Skill"),
    );

    let mut rules = vec![
        lang.t("Passive: while carried in a container, from pickup to death; a death leaves them in that kit's containers."),
        lang.t("Active: an artefact's key fires every ready copy of its kind."),
        lang.t("Spring's and Larkspur's actives add to their passive; an active Onyx replaces its passive."),
        lang.t("Firing restarts the whole cooldown of the kind's copies still cooling."),
        lang.t("Copies of a kind add up, Onyx's passive multiplies; at most 4 of a kind count."),
        lang.t("Onyx fired together reduce a hit once, 1 s longer for each other copy."),
        lang.t("Different kinds work side by side, each with its own key and cooldown."),
    ];
    rules.extend(item_lines(lang));
    let rules: String = rules
        .iter()
        .map(|line| format!("<li>{}</li>", escape(line)))
        .collect();

    let spawns = spawns();
    let keeper = SkillsTree::shared().skill(KEEPER).is_some();
    let mut where_rows = String::new();
    for artefact in all {
        let found: Vec<String> = spawns
            .iter()
            .filter(|(_, (_, kinds))| kinds.contains(&artefact.kind))
            .map(|(&(_, folder), &(name_key, _))| {
                format!(
                    "<a href=\"{}?lang={code}#{folder}\">{}</a>",
                    maps_page::PATH,
                    escape(&modifiers::text(lang, name_key))
                )
            })
            .collect();
        let share = keeper::SHARES
            .iter()
            .find(|(kind, _)| kind.dict_id() == artefact.dict_id)
            .map_or_else(|| "—".to_owned(), |&(_, share)| format!("{share}%"));
        let _ = write!(
            where_rows,
            "<tr><th>{}</th><td>{}</td>{}</tr>",
            escape(&name(lang, artefact)),
            if found.is_empty() {
                "—".to_owned()
            } else {
                found.join(", ")
            },
            if keeper {
                format!("<td class=num>{share}</td>")
            } else {
                String::new()
            },
        );
    }
    let keeper_head = SkillsTree::shared()
        .skill(KEEPER)
        .map_or_else(String::new, |skill| {
            format!("<th>{}</th>", escape(&modifiers::text(lang, &skill.name)))
        });
    let where_table = format!(
        "<div class=table-scroll><table class=artefact-table><thead><tr><th>{}</th><th>{}</th>\
         {keeper_head}</tr></thead><tbody>{where_rows}</tbody></table></div>",
        lang.t("Artefact"),
        lang.t("Maps"),
    );

    let skill_links: String = SKILLS
        .iter()
        .filter_map(|&id| skill_link(lang, id))
        .map(|link| format!("<li>{link}</li>"))
        .collect();
    let skills_section = if skill_links.is_empty() {
        String::new()
    } else {
        format!(
            "<section class=modifiers><h2>{}</h2><ul class=artefact-skills>{skill_links}</ul></section>",
            lang.t("Skills")
        )
    };

    format!(
        "<p class=breadcrumb><a href=\"/wiki?lang={code}\">{}</a> / {}</p>\
         <div class=wiki-heading><div><h1>{}</h1></div></div>{table}\
         <section class=modifiers><h2>{}</h2><ul class=mode-rules>{rules}</ul></section>\
         <section class=modifiers><h2>{}</h2>{where_table}</section>{skills_section}",
        lang.t("Wiki"),
        lang.t("Artefacts"),
        lang.t("Artefacts"),
        lang.t("Rules"),
        lang.t("Where"),
    )
}

/// A kind's row anchor.
fn anchor(kind: Kind) -> &'static str {
    match kind {
        Kind::Lifebone => "lifebone",
        Kind::Spring => "spring",
        Kind::Onyx => "onyx",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_names_every_artefact() {
        for lang in Lang::ALL {
            for artefact in artefacts::all() {
                let name = name(lang, artefact);
                assert!(
                    !name.is_empty() && !name.contains("artefact_"),
                    "{lang:?}: {name}"
                );
            }
        }
        let names: Vec<String> = artefacts::all().iter().map(|a| name(Lang::En, a)).collect();
        assert_eq!(names, ["Larkspur", "Spring", "Onyx"]);
    }

    #[test]
    fn every_artefact_has_its_row_numbers_and_spawns() {
        let page = body(Lang::En);
        for anchor in ["lifebone", "spring", "onyx"] {
            assert!(page.contains(&format!("<tr id=\"{anchor}\">")), "{anchor}");
        }
        // Each kind's numbers as its config has them, and each found on a map.
        for artefact in artefacts::all() {
            let shown = match &artefact.effects {
                Effects::Onyx {
                    passive, active, ..
                } => vec![
                    format!("Damage <b>{}</b>", percent(passive.damage.mul - 1.0)),
                    format!("Damage <b>{}</b>", percent(active.damage.mul - 1.0)),
                ],
                Effects::Spring { speed, active, .. } => vec![
                    format!("Movement speed <b>{}</b>", percent(speed.0)),
                    format!("Movement speed <b>{}</b>", percent(active.speed.0)),
                ],
                Effects::Lifebone { regeneration, .. } => vec![format!(
                    "Regeneration <b>+{} HP/s</b>",
                    number(regeneration[0].speed.add * 100.0)
                )],
            };
            for line in shown {
                assert!(page.contains(&line), "{line}");
            }
            assert!(
                spawns()
                    .values()
                    .any(|(_, kinds)| kinds.contains(&artefact.kind)),
                "{:?}",
                artefact.kind
            );
        }
        assert!(page.contains(&format!("{}?lang=en#", maps_page::PATH)));
        assert!(!page.contains('⟦'));
    }
}

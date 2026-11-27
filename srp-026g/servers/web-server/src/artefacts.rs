//! The wiki's artefacts, `/wiki/artefacts` on the 0.26g site: every artefact of the game data
//! with what it gives carried (its passive) and fired (its active), its active time, cooldown and
//! weight, its skill of the artefact tree, the rules of carrying several (docs/notes/artefacts.md,
//! exe patches 54 and 109), and what each activation leaves of its power (the wear rule,
//! [`ArtefactWear`]; docs/notes/artefacts.md "Wear from activation"): each kind's numbers after
//! 0 to [`ACTIVATIONS`] activations. Made from the server's own game data
//! ([`survarium::v026g::artefacts`], the skills tree), so it is the release's. The names
//! are the game's: its pickup hint names each artefact.

use std::fmt::Write;

use srp_validate::world::{Artefact as Kind, ArtefactWear};
use survarium::v026g::armour::BodyPart;
use survarium::v026g::artefacts::{
    self, Affine, ArtefactConfig, Effects, Modifier, Pool, Reduction,
};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::localization;
use survarium::v026g::skills::SkillsTree;

use crate::icons::IconSet;
use crate::modifiers::{self, number};
use crate::{Lang, equipment, escape, skills};

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

/// The activations the power tables go to: 0 to 10 (10.7 % left after the tenth).
const ACTIVATIONS: u16 = 10;

/// What a kind's effect lines are for.
#[derive(Clone, Copy, PartialEq)]
enum View {
    /// The artefact table: everything a fresh copy does, at most two decimals.
    Card,
    /// A row of a power table: only what wears, at a copy's power (`0..=1`), one decimal.
    Wear(f32),
}

impl View {
    fn power(self) -> f32 {
        match self {
            Self::Card => 1.0,
            Self::Wear(power) => power,
        }
    }

    fn number(self, value: f32) -> String {
        match self {
            Self::Card => number(value),
            Self::Wear(_) => {
                let text = format!("{value:.1}");
                text.trim_end_matches('0').trim_end_matches('.').to_owned()
            }
        }
    }

    /// A signed percentage of `fraction`.
    fn percent(self, fraction: f32) -> String {
        let sign = if fraction > 0.0 { "+" } else { "" };
        format!("{sign}{}%", self.number(fraction * 100.0))
    }

    /// A player modifier's change at this power (a worn Spring's node is its value times the
    /// power, the patch's `spring_value`): a percentage, or the factor `max(0, 1 + v)` when the
    /// fresh copy's is no share (stamina regeneration ×6), so a column keeps one form.
    fn modifier(self, Modifier(full): Modifier) -> String {
        let value = full * self.power();
        if full.abs() < 1.0 {
            self.percent(value)
        } else {
            format!("×{}", number((1.0 + value).max(0.0)))
        }
    }

    /// Stamina use at this power: the game floors spending at none, so a worn copy saves its
    /// power's share of all of it (the patch's `spring_value` of index 19: the config's -5
    /// counts as -1).
    fn spending(self, Modifier(full): Modifier) -> String {
        let value = match self {
            Self::Card => full,
            Self::Wear(power) => full.max(-1.0) * power,
        };
        format!("×{}", number((1.0 + value).max(0.0)))
    }

    /// `(x + add) × mul` at this power: `x + ((x + add) × mul - x) × power` (the patch's
    /// `at_power` of an Onyx's damage and piercing and a Lifebone's regeneration), written again
    /// as `(x + add') × mul'`.
    fn affine(self, Affine { add, mul }: Affine) -> Affine {
        let power = self.power();
        let worn = 1.0 + (mul - 1.0) * power;
        if power == 1.0 || worn == 0.0 {
            return Affine { add, mul };
        }
        Affine {
            add: add * mul * power / worn,
            mul: worn,
        }
    }
}

/// `(x + add) × mul` of health at `view`'s power: its terms that change something.
fn affine(lang: Lang, view: View, full: Affine) -> String {
    let Affine { add, mul } = view.affine(full);
    let mut terms = Vec::new();
    if add != 0.0 {
        let sign = if add > 0.0 { "+" } else { "" };
        terms.push(format!(
            "{sign}{} {}",
            view.number(add * 100.0),
            lang.tc("unit", "HP")
        ));
    }
    if mul != 1.0 {
        terms.push(view.percent(mul - 1.0));
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

fn onyx_lines(lang: Lang, view: View, reduction: Reduction) -> Vec<Line> {
    vec![
        (lang.t("Damage"), affine(lang, view, reduction.damage)),
        (
            lang.t("Penetration"),
            affine(lang, view, reduction.piercing),
        ),
    ]
}

/// What a kind gives carried, and fired (on top of it): all of it on the card; in a power
/// table only what wears, at that power (what it is against and the injuries cured stay).
fn lines(lang: Lang, effects: &Effects, view: View) -> (Vec<Line>, Vec<Line>) {
    match effects {
        Effects::Spring {
            speed,
            aim_time,
            active,
        } => {
            let mut passive = vec![(lang.t("Movement speed"), view.modifier(*speed))];
            passive.extend(aim_time.map(|aim| (lang.t("Aim time"), view.modifier(aim))));
            let mut fired = vec![(lang.t("Movement speed"), view.modifier(active.speed))];
            fired.extend(
                active
                    .aim_time
                    .map(|aim| (lang.t("Aim time"), view.modifier(aim))),
            );
            fired.push((
                lang.t("Stamina regeneration"),
                view.modifier(active.stamina_regeneration),
            ));
            fired.push((lang.t("Stamina use"), view.spending(active.stamina_use)));
            fired.extend(
                active
                    .stopping_power
                    .map(|stop| (lang.t("Slowdown from hits"), view.modifier(stop))),
            );
            (passive, fired)
        }
        Effects::Onyx {
            hit,
            parts,
            passive,
            active,
        } => {
            let mut carried = Vec::new();
            if view == View::Card {
                // Only parts it leaves out are worth a line.
                carried.push((lang.t("Against"), equipment::hit_name(lang, *hit)));
                if BodyPart::ALL.iter().any(|part| !parts.contains(part)) {
                    let mut names: Vec<String> = Vec::new();
                    for &part in parts {
                        let name = equipment::part_name(lang, part);
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                    carried.push((lang.t("Body parts"), names.join(", ")));
                }
            }
            // The active protects the same hits: said once, in the passive.
            carried.extend(onyx_lines(lang, view, *passive));
            (carried, onyx_lines(lang, view, *active))
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
                let (speed, delay, worn_limit) =
                    (view.affine(speed), view.affine(delay), view.affine(limit));
                if speed.add != 0.0 || speed.mul != 1.0 {
                    let mut terms = Vec::new();
                    if speed.add != 0.0 {
                        let sign = if speed.add > 0.0 { "+" } else { "" };
                        let per_second = lang.tc("unit", "HP/s");
                        terms.push(format!(
                            "{sign}{} {per_second}",
                            view.number(speed.add * 100.0)
                        ));
                    }
                    if speed.mul != 1.0 {
                        terms.push(view.percent(speed.mul - 1.0));
                    }
                    carried.push((of(lang.t("Regeneration")), terms.join(" ")));
                }
                if delay.add != 0.0 || delay.mul != 1.0 {
                    let mut terms = Vec::new();
                    if delay.add != 0.0 {
                        let sign = if delay.add > 0.0 { "+" } else { "" };
                        terms.push(format!(
                            "{sign}{} {}",
                            view.number(delay.add),
                            lang.tc("unit", "s")
                        ));
                    }
                    if delay.mul != 1.0 {
                        terms.push(view.percent(delay.mul - 1.0));
                    }
                    carried.push((of(lang.t("Regeneration delay")), terms.join(" ")));
                }
                // The limit is a share of the pool's maximum, at most all of it. A worn copy
                // raises the pool's own limit by its power's share of what a fresh one adds.
                let limit_terms = |Affine { add, mul }: Affine| {
                    let mut terms = Vec::new();
                    if add != 0.0 {
                        terms.push(view.percent(add));
                    }
                    if mul != 1.0 {
                        terms.push(format!("×{}", number(mul)));
                    }
                    terms.join(" ")
                };
                match view {
                    View::Card if limit.add >= 1.0 && limit.mul >= 1.0 => {
                        carried.push((of(lang.t("Regenerates up to")), "100%".to_owned()));
                    }
                    _ if limit.add == 0.0 && limit.mul == 1.0 => {}
                    View::Card => {
                        carried.push((of(lang.t("Regenerates up to")), limit_terms(limit)));
                    }
                    View::Wear(_) => {
                        carried.push((of(lang.t("Regeneration limit")), limit_terms(worn_limit)));
                    }
                }
            }
            let fired = match view {
                // Exe patch 54: the active fills every pool to its maximum.
                View::Card => {
                    let mut fired = vec![(lang.t("Health"), "100%".to_owned())];
                    if !cures.is_empty() {
                        fired.push((lang.t("Cures injuries"), pool_names(lang, cures)));
                    }
                    fired
                }
                // A worn copy restores its power's share of each pool's missing health (the
                // patch's `heal_amount`); it cures the same injuries.
                View::Wear(power) => vec![(
                    lang.t("Missing health restored"),
                    format!("{}%", view.number(power * 100.0)),
                )],
            };
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
    crate::versioned(&format!("/skills/icons/{id}.webp"))
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

/// An artefact's lobby icon, as its row shows it.
fn picture(artefact: &ArtefactConfig) -> String {
    crate::icons::url(
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
    })
}

/// The skills of the artefact tree that work through a copy, at its power: Spring II, Onyx II,
/// Larkspur II (the patch's reload nodes, landing and energy), Resonance and Cascade (the
/// source copy's).
const WORN_SKILLS: [u16; 5] = [531, 532, 533, 541, 542];

/// A skill's name in `lang`, none without the skill.
fn skill_name(lang: Lang, id: u16) -> Option<String> {
    SkillsTree::shared()
        .skill(id)
        .map(|skill| modifiers::text(lang, &skill.name))
}

/// A copy's power after `uses` successful activations: what its passive gives now and its next
/// activation fires with (the game's own rule).
fn power(uses: u16) -> f32 {
    ArtefactWear::new(uses).power()
}

/// The power section: the rule in words, the power after each activation, and each kind's
/// numbers by activation, each kind's table folded.
fn wear_section(lang: Lang) -> String {
    let steps: String = (0..=ACTIVATIONS)
        .map(|uses| {
            format!(
                "<li><small>{uses}</small><b>{}%</b></li>",
                View::Wear(1.0).number(power(uses) * 100.0)
            )
        })
        .collect();
    let steps = format!(
        "<ol class=wear-steps><li class=wear-steps-head><small>{}</small><b>{}</b></li>{steps}</ol>",
        escape(&lang.t("Activations")),
        escape(&lang.tc("artefact", "Power")),
    );

    let skills: Vec<String> = WORN_SKILLS
        .iter()
        .filter_map(|&id| skill_name(lang, id))
        .collect();
    let mut text = vec![
        lang.t("Each activation leaves an artefact 80% of the power it had: 100%, 80%, 64%, 51.2% and so on. Its passive drops at once; the activation itself still works at the power from before it."),
        lang.t("Only a copy a press fires loses power, each copy its own share. One cooling down, already active or past the 4 that count loses nothing."),
        lang.t("It never runs out: the copy stays usable all match. Active time, cooldown and weight stay the same, and Larkspur cures injuries on every activation."),
    ];
    if let (Some(courier), Some(keeper)) = (skill_name(lang, 534), skill_name(lang, 521)) {
        text.push(lang.tf(
            "The power stays with the copy: through deaths, kit changes ({courier} too) and in another player's hands. A new copy starts at 100%: one from an anomaly, and the one {keeper} gives.",
            &[("courier", &courier), ("keeper", &keeper)],
        ));
    }
    if !skills.is_empty() {
        text.push(lang.tf(
            "The skills that work through a copy wear with it: {skills}.",
            &[("skills", &skills.join(", "))],
        ));
    }
    let text: String = text
        .iter()
        .map(|line| format!("<p>{}</p>", escape(line)))
        .collect();

    let mut kinds = String::new();
    for artefact in artefacts::all() {
        let rows: Vec<(u16, Vec<Line>, Vec<Line>)> = (0..=ACTIVATIONS)
            .map(|uses| {
                let (carried, fired) = lines(lang, &artefact.effects, View::Wear(power(uses)));
                (uses, carried, fired)
            })
            .collect();
        let Some((_, carried, fired)) = rows.first() else {
            continue;
        };
        let head = |lines: &[Line]| -> String {
            lines
                .iter()
                .map(|(label, _)| format!("<th>{}</th>", escape(label)))
                .collect()
        };
        let mut body = String::new();
        for (uses, carried, fired) in &rows {
            let _ = write!(
                body,
                "<tr><th>{uses}</th><td>{}%</td>",
                View::Wear(1.0).number(power(*uses) * 100.0)
            );
            for (_, value) in carried.iter().chain(fired) {
                let _ = write!(body, "<td>{}</td>", escape(value));
            }
            body.push_str("</tr>");
        }
        let limit = lang.t("Regeneration limit");
        let limit_note =
            if carried.iter().any(|(label, _)| label.starts_with(&limit)) {
                format!(
                    "<p class=wiki-note>{}</p>",
                    escape(&lang.t(
                        "Regeneration limit adds to each body part's own limit, never past full."
                    ))
                )
            } else {
                String::new()
            };
        let _ = write!(
            kinds,
            "<details class=wear-kind id=\"wear-{}\"><summary>{}<span>{}</span></summary>\
             <div class=table-scroll><table class=\"artefact-table wear-table\"><thead>\
             <tr><th rowspan=2>{}</th><th rowspan=2>{}</th><th colspan={}>{}</th><th colspan={}>{}</th></tr>\
             <tr>{}{}</tr></thead><tbody>{body}</tbody></table></div>{limit_note}</details>",
            anchor(artefact.kind),
            picture(artefact),
            escape(&name(lang, artefact)),
            escape(&lang.t("Activations")),
            escape(&lang.tc("artefact", "Power")),
            carried.len(),
            escape(&lang.t("Passive")),
            fired.len(),
            escape(&lang.t("Next activation")),
            head(carried),
            head(fired),
        );
    }

    format!(
        "<section class=\"modifiers artefact-wear\" id=wear><h2>{}</h2><div class=wear-text>{text}</div>\
         {steps}<p class=wiki-note>{}</p>{kinds}</section>",
        escape(&lang.t("Power per activation")),
        escape(&lang.t("A row: after that many activations, the passive now and what the next activation gives.")),
    )
}

/// The page's body in `lang`.
pub(crate) fn body(lang: Lang) -> String {
    let code = lang.code();
    let seconds = modifiers::text(lang, "st_item_props_sec");
    let kg = modifiers::text(lang, "st_item_props_kg");
    let all = artefacts::all();

    let mut rows = String::new();
    for artefact in all {
        let (carried, fired) = lines(lang, &artefact.effects, View::Card);
        let picture = picture(artefact);
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

    let rules = [
        lang.t("Carried in a container: passive. A death leaves them in that kit."),
        lang.t("A key fires every ready copy of its kind and restarts the cooldown of the ones still cooling."),
        lang.t("Copies of a kind add up (Onyx's passive multiplies); at most 4 count."),
        lang.t("Spring's and Larkspur's actives add to the passive; Onyx's replaces it."),
        lang.t("Onyx fired together: one reduction, 1 s longer per extra copy."),
    ];
    let rules: String = rules
        .iter()
        .map(|line| format!("<li>{}</li>", escape(line)))
        .collect();

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
         <section class=modifiers><h2>{}</h2><ul class=mode-rules>{rules}</ul></section>{}{skills_section}",
        lang.t("Wiki"),
        lang.t("Artefacts"),
        lang.t("Artefacts"),
        lang.t("Rules"),
        wear_section(lang),
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
    fn every_artefact_has_its_row_and_numbers() {
        let page = body(Lang::En);
        for anchor in ["lifebone", "spring", "onyx"] {
            assert!(page.contains(&format!("<tr id=\"{anchor}\">")), "{anchor}");
        }
        // Each kind's numbers as its config has them.
        for artefact in artefacts::all() {
            let shown = match &artefact.effects {
                Effects::Onyx {
                    passive, active, ..
                } => vec![
                    format!(
                        "Damage <b>{}</b>",
                        View::Card.percent(passive.damage.mul - 1.0)
                    ),
                    format!(
                        "Damage <b>{}</b>",
                        View::Card.percent(active.damage.mul - 1.0)
                    ),
                ],
                Effects::Spring { speed, active, .. } => vec![
                    format!("Movement speed <b>{}</b>", View::Card.percent(speed.0)),
                    format!(
                        "Movement speed <b>{}</b>",
                        View::Card.percent(active.speed.0)
                    ),
                ],
                Effects::Lifebone { regeneration, .. } => vec![format!(
                    "Regeneration <b>+{} HP/s</b>",
                    number(regeneration[0].speed.add * 100.0)
                )],
            };
            for line in shown {
                assert!(page.contains(&line), "{line}");
            }
        }
        assert!(!page.contains('⟦'));
    }

    #[test]
    fn a_worn_term_is_the_patchs_share_of_the_fresh_change() {
        // The patch's at_power: x + ((x + add) × mul - x) × power.
        for full in [
            Affine {
                add: 0.0,
                mul: 0.25,
            },
            Affine { add: 0.1, mul: 1.0 },
            Affine { add: 1.0, mul: 1.0 },
            Affine {
                add: -2.0,
                mul: 0.5,
            },
        ] {
            for uses in 0..=ACTIVATIONS {
                let worn = View::Wear(power(uses)).affine(full);
                for x in [0.03_f32, 0.5, 8.0] {
                    let patch = x + ((x + full.add) * full.mul - x) * power(uses);
                    assert!(
                        ((x + worn.add) * worn.mul - patch).abs() < 1e-5,
                        "{full:?} {uses} {x}"
                    );
                }
            }
        }
    }

    #[test]
    fn each_kind_has_its_numbers_by_activation() {
        let page = body(Lang::En);
        for anchor in ["lifebone", "spring", "onyx"] {
            assert!(
                page.contains(&format!("<details class=wear-kind id=\"wear-{anchor}\">")),
                "{anchor}"
            );
        }
        for step in ["100%", "80%", "64%", "51.2%", "41%", "32.8%", "10.7%"] {
            assert!(page.contains(&format!("<b>{step}</b>")), "{step}");
        }
        // One column a line: every row has the fresh copy's lines, in its order.
        for artefact in artefacts::all() {
            let labels = |uses| {
                let (carried, fired) = lines(Lang::En, &artefact.effects, View::Wear(power(uses)));
                carried
                    .into_iter()
                    .chain(fired)
                    .map(|(label, _)| label)
                    .collect::<Vec<_>>()
            };
            assert!(!labels(0).is_empty());
            for uses in 1..=ACTIVATIONS {
                assert_eq!(labels(uses), labels(0), "{:?} after {uses}", artefact.kind);
            }
        }
        // After one activation: 80 % of each fresh change (Spring's, Onyx's from the config).
        let after_one = |artefact: &ArtefactConfig| {
            let (carried, fired) = lines(Lang::En, &artefact.effects, View::Wear(power(1)));
            carried.into_iter().chain(fired).collect::<Vec<_>>()
        };
        let one = |fraction: f32| {
            let text = format!("{:.1}", fraction * 0.8 * 100.0);
            let text = text.trim_end_matches('0').trim_end_matches('.');
            if fraction > 0.0 {
                format!("+{text}%")
            } else {
                format!("{text}%")
            }
        };
        for artefact in artefacts::all() {
            let shown = after_one(artefact);
            let has = |label: &str, value: String| {
                assert!(
                    shown.contains(&(label.to_owned(), value.clone())),
                    "{label} {value}: {shown:?}"
                );
            };
            match &artefact.effects {
                Effects::Spring { speed, active, .. } => {
                    has("Movement speed", one(speed.0));
                    has("Movement speed", one(active.speed.0));
                    has(
                        "Stamina use",
                        format!("×{}", number(1.0 + active.stamina_use.0.max(-1.0) * 0.8)),
                    );
                }
                Effects::Onyx {
                    passive, active, ..
                } => {
                    has("Damage", one(passive.damage.mul - 1.0));
                    has("Damage", one(active.damage.mul - 1.0));
                }
                Effects::Lifebone { .. } => has("Missing health restored", "80%".to_owned()),
            }
        }
    }
}

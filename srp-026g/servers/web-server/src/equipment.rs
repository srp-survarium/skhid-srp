//! The wiki's equipment, `/wiki/equipment` on the 0.26g site: every armour
//! piece with its lobby icon, protection per body part and hit type, weight,
//! the slots it takes, an oxygen tank's time and gas filter, whether a helmet
//! flies off on a fatal headshot, and what it can roll and comes with (the
//! modifiers page's lists, [`crate::modifiers::item_lists`]). What it shows
//! is what changes play (docs/notes/armour-params.md). Made from the server's
//! own game data ([`survarium::v026g::armour`], `item_params`), so it is the
//! release's equipment, the newest pieces included.
//!
//! The page's script filters by slot (one click, kept in the address:
//! `?slot=helmet`), faction and level, finds by name, sorts and groups by
//! set or slot. A set is the pieces of one faction and tier: their configs
//! share `<family>_<slot>_<tier>` (renesanse_torso_4, renesanse_helmet_4).

use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::OnceLock;

use survarium::v026g::armour::{self, Armour, BodyPart, HitType};
use survarium::v026g::dictionary::{Dictionary, Item};
use survarium::v026g::item_params::{self, ItemParams};

use crate::icons::IconSet;
use crate::modifiers::{self, Group};
use crate::{Lang, escape};

const SCRIPT: &[u8] = include_bytes!("../wiki/equipment.js");
const SCRIPT_PATH: &str = "/wiki/equipment.js";

/// The page's script.
pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    (path == SCRIPT_PATH).then_some(("text/javascript; charset=utf-8", SCRIPT))
}

fn script_url() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| {
        let digest: String = openssl::sha::sha256(SCRIPT)[..6]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{SCRIPT_PATH}?v={digest}")
    })
}

/// The headwear edit list (content `headwear-1.json`, which the release lays
/// in the players' data): the helmets whose model it rebinds to the
/// render-only bone fly off on a fatal headshot (exe patch 79, #699).
const HEADWEAR: &str = include_str!("../../../scripts/content-edit/headwear-1.json");

/// The models (`data.base_model`) of the helmets that fly off: those of the
/// list's `rigid_headwear` surfaces.
fn flying_models() -> &'static [String] {
    static MODELS: OnceLock<Vec<String>> = OnceLock::new();
    MODELS.get_or_init(|| {
        let rigid = HEADWEAR
            .split_once("\"rigid_headwear\"")
            .map_or("", |(_, rest)| rest);
        let mut models: Vec<String> = rigid
            .match_indices("\"models/")
            .filter_map(|(at, _)| {
                let path = &rigid[at + "\"models/".len()..];
                Some(path[..path.find(".skinned_model")?].to_owned())
            })
            .collect();
        models.sort();
        models.dedup();
        models
    })
}

pub(crate) fn flies_off(armour: &Armour) -> bool {
    armour
        .model
        .as_ref()
        .is_some_and(|model| flying_models().contains(model))
}

/// The best protection a piece has against `hits` on any part.
fn best(armour: &Armour, hits: &[HitType]) -> f32 {
    armour
        .protection
        .iter()
        .filter(|(_, hit, _)| hits.contains(hit))
        .map(|&(_, _, armor)| armor)
        .fold(0.0, f32::max)
}

const ANOMALIES: [HitType; 4] = [
    HitType::Fire,
    HitType::Radiation,
    HitType::Shock,
    HitType::Toxic,
];

/// The slots in the lobby's order, each with its address word.
const SLOTS: [(Group, &str); 7] = [
    (Group::Helmet, "helmet"),
    (Group::Mask, "mask"),
    (Group::Jacket, "body"),
    (Group::Back, "backpack"),
    (Group::Gloves, "gloves"),
    (Group::Pants, "pants"),
    (Group::Boots, "boots"),
];

pub(crate) fn slot_word(group: Group) -> &'static str {
    SLOTS
        .iter()
        .find(|(slot, _)| *slot == group)
        .map_or("other", |&(_, word)| word)
}

/// The lobby's name of faction `faction` (`parameters.faction`): its
/// `factions_dict` row's name, or the lobby's independent traders for none.
pub(crate) fn faction_name(lang: Lang, faction: u8) -> String {
    let key = Dictionary::shared()
        .factions
        .iter()
        .find(|row| row.id == faction)
        .map_or("st_non_faction_items", |row| row.name.as_str());
    modifiers::text(lang, key)
}

/// The set a piece belongs to: its faction and tier when its config is
/// named `<family>_<slot>_<tier>` (a premium copy with its plain piece),
/// else the piece alone.
fn set_key(item: &Item, params: &ItemParams) -> String {
    let stem = item
        .cfg_name
        .rsplit('/')
        .next()
        .unwrap_or(&item.cfg_name)
        .trim_end_matches(".options")
        .trim_end_matches("_premium");
    let mut words = stem.rsplit('_');
    match (
        words.next().and_then(|tier| tier.parse::<u32>().ok()),
        words.next(),
    ) {
        (Some(tier), Some(_slot)) => format!("{}-{tier}", params.faction),
        _ => format!("item-{}", item.name),
    }
}

/// A part's name, both sides together.
fn part_name(lang: Lang, part: BodyPart) -> String {
    match part {
        BodyPart::Brain => lang.t("Head"),
        BodyPart::Face => lang.t("Face"),
        BodyPart::Front => lang.t("Chest"),
        BodyPart::Back => lang.t("Back"),
        BodyPart::LeftForearm | BodyPart::RightForearm => lang.t("Arms"),
        BodyPart::LeftArm | BodyPart::RightArm => lang.t("Hands"),
        BodyPart::LeftHip | BodyPart::RightHip => lang.t("Legs"),
        BodyPart::LeftFoot | BodyPart::RightFoot => lang.t("Feet"),
    }
}

fn hit_name(lang: Lang, hit: HitType) -> String {
    match hit {
        HitType::Ranged => lang.t("Bullets"),
        HitType::Slash => lang.t("Melee"),
        HitType::Explosive => lang.t("Explosions"),
        HitType::Fire => lang.t("Fire"),
        HitType::Radiation => lang.t("Radiation"),
        HitType::Shock => lang.t("Electricity"),
        HitType::Toxic => lang.t("Toxins"),
    }
}

/// A fraction as the lobby's cards print armour: a whole number of 100.
pub(crate) fn hundredths(value: f32) -> String {
    format!("{:.0}", value * 100.0)
}

/// Physical hits: one value in every config so far, the lobby's Armour.
const PHYSICAL: [HitType; 3] = [HitType::Ranged, HitType::Slash, HitType::Explosive];

/// One value for `hits` in `values`: `Some(None)` when it has none of them,
/// `None` when they differ.
fn one_value(values: &BTreeMap<HitType, f32>, hits: &[HitType]) -> Option<Option<f32>> {
    let mut found = hits.iter().filter_map(|hit| values.get(hit).copied());
    let first = found.next();
    found.all(|value| Some(value) == first).then_some(first)
}

/// The protection table: a row per part (left and right as one, parts with
/// the same values together). Every config so far gives a part one value
/// for its physical types and one for its anomaly types: then two columns,
/// Armour and Isolation as the lobby says; a config that does not gets a
/// column per hit type.
pub(crate) struct Protection {
    pub(crate) heads: Vec<String>,
    /// The parts with those values, and the values (fractions).
    pub(crate) rows: Vec<(Vec<String>, Vec<Option<f32>>)>,
}

/// `armour`'s protection table; `None` for a piece that protects nothing.
pub(crate) fn protection(lang: Lang, armour: &Armour) -> Option<Protection> {
    let mut by_part: BTreeMap<BodyPart, BTreeMap<HitType, f32>> = BTreeMap::new();
    for &(part, hit, value) in &armour.protection {
        by_part.entry(part).or_default().insert(hit, value);
    }
    if by_part.is_empty() {
        return None;
    }
    let paired = by_part.values().all(|values| {
        one_value(values, &PHYSICAL).is_some() && one_value(values, &ANOMALIES).is_some()
    });
    let hits: Vec<HitType> = HitType::ALL
        .into_iter()
        .filter(|hit| by_part.values().any(|row| row.contains_key(hit)))
        .collect();
    let heads: Vec<String> = if paired {
        vec![lang.t("Armour"), lang.t("Isolation")]
    } else {
        hits.iter().map(|&hit| hit_name(lang, hit)).collect()
    };
    let cells = |values: &BTreeMap<HitType, f32>| -> Vec<Option<f32>> {
        if paired {
            vec![
                one_value(values, &PHYSICAL).flatten(),
                one_value(values, &ANOMALIES).flatten(),
            ]
        } else {
            hits.iter().map(|hit| values.get(hit).copied()).collect()
        }
    };
    let mut rows: Vec<(Vec<String>, Vec<Option<f32>>)> = Vec::new();
    for (part, values) in &by_part {
        let row = cells(values);
        let name = part_name(lang, *part);
        match rows.iter_mut().find(|(_, other)| *other == row) {
            Some((names, _)) => {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            None => rows.push((vec![name], row)),
        }
    }
    Some(Protection { heads, rows })
}

/// The protection table of `armour` as its card shows it.
fn protection_html(lang: Lang, armour: &Armour) -> String {
    let Some(Protection { heads, rows }) = protection(lang, armour) else {
        return String::new();
    };
    let mut html = format!(
        "<section><h2>{}</h2><div class=table-scroll><table class=protection><thead><tr><th></th>",
        lang.t("Protection")
    );
    for head in &heads {
        let _ = write!(html, "<th>{}</th>", escape(head));
    }
    html.push_str("</tr></thead><tbody>");
    for (names, row) in rows {
        let _ = write!(html, "<tr><th>{}</th>", escape(&names.join(", ")));
        for cell in row {
            let _ = write!(
                html,
                "<td>{}</td>",
                cell.map(hundredths).unwrap_or_default()
            );
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table></div></section>");
    html
}

/// The profile slot `slot` (0..=6: helmet, mask, torso, back, legs, gloves,
/// boots) as the lobby names it.
fn profile_slot_name(lang: Lang, slot: u8) -> Option<String> {
    let group = [
        Group::Helmet,
        Group::Mask,
        Group::Jacket,
        Group::Back,
        Group::Pants,
        Group::Gloves,
        Group::Boots,
    ]
    .get(usize::from(slot))?;
    Some(modifiers::text(lang, group.name_key()))
}

/// The card's facts: level, matchmaking level when it differs, faction, the
/// slots its model takes, an oxygen tank's time and gas filter, the fly-off.
fn about_html(lang: Lang, params: &ItemParams, armour: &Armour, faction: &str) -> String {
    let mut rows = vec![
        (lang.t("Level"), params.item_level.to_string()),
        (lang.t("Faction"), faction.to_owned()),
    ];
    if params.match_making_level != params.item_level {
        rows.push((
            lang.t("Matchmaking level"),
            params.match_making_level.to_string(),
        ));
    }
    let taken: Vec<String> = params
        .covered_slots
        .iter()
        .filter_map(|&slot| profile_slot_name(lang, slot))
        .collect();
    if !taken.is_empty() {
        rows.push((lang.t("Also takes"), taken.join(", ")));
    }
    let seconds = modifiers::text(lang, "st_item_props_sec");
    if let Some(time) = armour.oxygen {
        rows.push((lang.t("Oxygen"), format!("{time} {seconds}")));
    }
    for filter in &armour.gas {
        rows.push((
            lang.t("Gas filter"),
            lang.tf("{0}: hits up to {1} stopped, the rest ×{2}", &[("0", &part_name(lang, filter.part)), ("1", &filter.threshold), ("2", &filter.hit_coeff)]),
        ));
    }
    if flies_off(armour) {
        rows.push((
            lang.t("Fatal headshot"),
            lang.t("flies off"),
        ));
    }
    let rows: String = rows
        .iter()
        .map(|(label, value)| {
            format!(
                "<div><dt>{}</dt><dd>{}</dd></div>",
                escape(label),
                escape(value)
            )
        })
        .collect();
    format!(
        "<section><h2>{}</h2><dl>{rows}</dl></section>",
        lang.t("About it")
    )
}

/// One piece's row and card.
fn piece_html(
    lang: Lang,
    item: &Item,
    params: &ItemParams,
    armour: &Armour,
    group: Group,
) -> String {
    let name = modifiers::item_name(lang, item.dict_id);
    let slot = modifiers::text(lang, group.name_key());
    let faction = faction_name(lang, params.faction);
    let picture = crate::icons::url(IconSet::Item, &item.name).map_or_else(String::new, |url| {
        format!(
            "<img class=weapon-image src=\"{}\" alt=\"\" width=128 height=62 loading=lazy>",
            escape(&url)
        )
    });
    let bullets = best(armour, &PHYSICAL);
    let anomalies = best(armour, &ANOMALIES);
    let quick = |label: String, value: String, unit: &str| {
        format!(
            "<div><dt title=\"{0}\">{0}</dt><dd>{1}<small>{2}</small></dd></div>",
            escape(&label),
            escape(&value),
            escape(unit)
        )
    };
    let premium = if item.is_premium {
        format!(" · {}", lang.t("premium"))
    } else {
        String::new()
    };
    let lists = modifiers::item_lists(
        lang,
        item.dict_id,
        &format!("/wiki/modifiers?lang={}", lang.code()),
    );
    format!(
        "<details class=\"weapon gear\" id=\"{key}\" data-slot={slot_word} data-faction={faction_id} \
         data-level={level} data-name=\"{lower}\" data-weight={weight} data-armour={bullets} \
         data-isolation={anomalies} data-set=\"{set}\"><summary>{picture}\
         <span class=weapon-title><strong>{name_html}</strong><span>{slot_html} · {level_word} {level} · {faction_html}{premium}</span></span>\
         <dl class=quick-stats>{q1}{q2}{q3}</dl>\
         <span class=expand-label>{details} <span aria-hidden=true>+</span></span></summary>\
         <div class=weapon-details>{protection}{about}<section class=gear-modifiers>{lists}\
         <p><a href=\"/wiki/modifiers?lang={code}&amp;item={key}#item\">{all_mods} →</a></p></section></div></details>",
        key = escape(&item.name),
        slot_word = slot_word(group),
        faction_id = params.faction,
        level = params.item_level,
        lower = escape(&name.to_lowercase()),
        weight = params.weight,
        set = escape(&set_key(item, params)),
        name_html = escape(&name),
        slot_html = escape(&slot),
        level_word = lang.t("level"),
        faction_html = escape(&faction),
        q1 = quick(lang.t("Armour"), hundredths(bullets), ""),
        q2 = quick(lang.t("Isolation"), hundredths(anomalies), ""),
        q3 = quick(
            lang.t("Weight"),
            format!("{}", params.weight),
            &modifiers::text(lang, "st_item_props_kg")
        ),
        details = lang.t("Details"),
        protection = protection_html(lang, armour),
        about = about_html(lang, params, armour, &faction),
        all_mods = lang.t("Its modifiers"),
        code = lang.code(),
    )
}

/// Every armour piece with its params and group, in the lobby's slot order,
/// then by level.
pub(crate) fn pieces() -> Vec<(&'static Item, &'static ItemParams, &'static Armour, Group)> {
    let dictionary = Dictionary::shared();
    let mut rows: Vec<_> = armour::all()
        .iter()
        .filter_map(|armour| {
            let item = dictionary.item(armour.dict_id)?;
            let group = Group::of(item.category)?;
            Some((
                item,
                item_params::item_params(armour.dict_id)?,
                armour,
                group,
            ))
        })
        .collect();
    rows.sort_by_key(|&(item, params, _, group)| {
        (
            SLOTS.iter().position(|&(slot, _)| slot == group),
            params.item_level,
            item.dict_id,
        )
    });
    rows
}

/// The page's body in `lang`.
pub(crate) fn body(lang: Lang) -> String {
    let code = lang.code();
    let rows = pieces();
    let mut slots = format!(
        "<div class=slot-filter role=group aria-label=\"{}\"><button type=button data-slot=\"\" aria-pressed=true>{}</button>",
        lang.t("Slot"),
        lang.t("All")
    );
    for &(group, word) in &SLOTS {
        let name = escape(&modifiers::text(lang, group.name_key()));
        let _ = write!(
            slots,
            "<button type=button data-slot={word} aria-pressed=false title=\"{name}\">{}<span>{name}</span></button>",
            modifiers::glyph(IconSet::Group, &group.category().to_string())
        );
    }
    slots.push_str("</div>");
    let mut factions: Vec<u8> = rows
        .iter()
        .map(|(_, params, _, _)| params.faction)
        .collect();
    factions.sort_unstable();
    factions.dedup();
    let mut levels: Vec<u8> = rows
        .iter()
        .map(|(_, params, _, _)| params.item_level)
        .collect();
    levels.sort_unstable();
    levels.dedup();
    let options = |values: &[u8], selected: Option<u8>| -> String {
        values
            .iter()
            .map(|level| {
                format!(
                    "<option value={level}{}>{level}</option>",
                    if Some(*level) == selected {
                        " selected"
                    } else {
                        ""
                    }
                )
            })
            .collect()
    };
    let faction_options: String = factions
        .iter()
        .map(|&faction| {
            format!(
                "<option value={faction}>{}</option>",
                escape(&faction_name(lang, faction))
            )
        })
        .collect();
    let tools = format!(
        "<div class=\"catalogue-tools gear-tools\" hidden>\
         <label>{find}<input type=search id=gear-search placeholder=\"{placeholder}\" autocomplete=off></label>\
         <label>{faction}<select id=gear-faction><option value=\"\">{all_factions}</option>{faction_options}</select></label>\
         <label>{from}<select id=gear-level-from>{from_options}</select></label>\
         <label>{to}<select id=gear-level-to>{to_options}</select></label>\
         <label>{sort}<select id=gear-sort><option value=level>{by_level}</option><option value=name>{by_name}</option>\
         <option value=weight>{by_weight}</option><option value=armour>{by_armour}</option><option value=isolation>{by_isolation}</option></select></label>\
         <label>{group}<select id=gear-group><option value=\"\">{no_groups}</option><option value=set>{by_set}</option><option value=slot>{by_slot}</option></select></label></div>",
        find = lang.t("Find"),
        placeholder = lang.t("Search by name…"),
        faction = lang.t("Faction"),
        all_factions = lang.t("All factions"),
        from = lang.t("Level from"),
        to = lang.t("to"),
        from_options = options(&levels, levels.first().copied()),
        to_options = options(&levels, levels.last().copied()),
        sort = lang.t("Sort by"),
        by_level = lang.t("Level"),
        by_name = lang.t("Name"),
        by_weight = lang.t("Weight: low to high"),
        by_armour = lang.t("Armour: high to low"),
        by_isolation = lang.t("Isolation: high to low"),
        group = lang.t("Group"),
        no_groups = lang.t("No groups"),
        by_set = lang.t("By set"),
        by_slot = lang.t("By slot"),
    );
    // The sets' names, for the script's group headings: a set's body armour
    // names it, else its first piece.
    let mut set_names: BTreeMap<String, (bool, String)> = BTreeMap::new();
    for &(item, params, _, group) in &rows {
        let key = set_key(item, params);
        let named = (
            group == Group::Jacket,
            modifiers::item_name(lang, item.dict_id),
        );
        let entry = set_names.entry(key).or_insert_with(|| named.clone());
        if named.0 && !entry.0 {
            *entry = named;
        }
    }
    let set_list: String = set_names
        .iter()
        .map(|(key, (_, name))| format!("<li data-set=\"{}\">{}</li>", escape(key), escape(name)))
        .collect();
    let list: String = rows
        .iter()
        .map(|&(item, params, armour, group)| piece_html(lang, item, params, armour, group))
        .collect();
    format!(
        "<p class=breadcrumb><a href=\"/wiki?lang={code}\">{wiki}</a> / {title}</p>\
         <div class=wiki-heading><div><p class=eyebrow>{eyebrow}</p><h1>{title}</h1></div>\
         <span class=catalogue-count role=status aria-live=polite data-count data-label=\"{pieces}\">{count} {pieces}</span></div>\
         {slots}{tools}<ul id=gear-sets hidden>{set_list}</ul>\
         <div id=gear-list>{list}</div><p id=gear-empty hidden>{empty}</p><script src=\"{script}\"></script>",
        wiki = lang.t("Wiki"),
        title = lang.t("Equipment"),
        eyebrow = lang.t("Equipment reference"),
        pieces = lang.t("pieces"),
        count = rows.len(),
        empty = lang.t("Nothing matches."),
        script = script_url(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_piece_is_listed_with_its_icon_and_protection() {
        let rows = pieces();
        assert!(rows.len() > 100, "{}", rows.len());
        let page = body(Lang::En);
        assert_eq!(
            page.matches("<details class=\"weapon gear\"").count(),
            rows.len()
        );
        assert_eq!(
            page.matches("class=weapon-image").count(),
            rows.len(),
            "an icon a piece"
        );
        assert!(page.contains("<table class=protection>"));
        assert!(page.contains("data-slot=helmet"));
        // Each card links to its kinds on the modifiers page.
        assert!(page.contains("href=\"/wiki/modifiers?lang=en#kind-"));
    }

    #[test]
    fn protection_is_armour_and_isolation_when_the_types_agree() {
        let rows = pieces();
        let (_, _, torso, _) = rows
            .iter()
            .find(|&&(_, _, armour, group)| group == Group::Jacket && !armour.protection.is_empty())
            .unwrap();
        let table = protection_html(Lang::En, torso);
        assert!(
            table.contains("<th></th><th>Armour</th><th>Isolation</th>"),
            "{table}"
        );
        // A config whose types differ gets a column a type.
        let mut odd = (*torso).clone();
        odd.protection[0].2 += 0.01;
        assert!(protection_html(Lang::En, &odd).contains("<th>Bullets</th>"));
    }

    #[test]
    fn the_faction_filter_lists_the_factions_that_have_items() {
        let page = body(Lang::En);
        let mut factions: Vec<u8> = pieces()
            .iter()
            .map(|(_, params, _, _)| params.faction)
            .collect();
        factions.sort_unstable();
        factions.dedup();
        for faction in factions {
            assert!(page.contains(&format!(
                "<option value={faction}>{}</option>",
                faction_name(Lang::En, faction)
            )));
        }
        assert!(
            !faction_name(Lang::En, 1).starts_with("st_"),
            "a name, not a key"
        );
    }

    #[test]
    fn a_set_is_a_faction_and_tier() {
        let dictionary = Dictionary::shared();
        let set_of = |name: &str| {
            let item = dictionary.item_named(name).unwrap();
            set_key(item, item_params::item_params(item.dict_id).unwrap())
        };
        assert_eq!(set_of("ren_torso_4"), set_of("ren_helmet_4"));
        assert_ne!(set_of("ren_torso_4"), set_of("ren_torso_5"));
    }

    #[test]
    fn both_languages_have_the_slot_buttons() {
        for lang in [Lang::En, Lang::Uk] {
            let page = body(lang);
            assert!(page.contains(&format!("href=\"/wiki?lang={}\"", lang.code())));
        }
    }
}

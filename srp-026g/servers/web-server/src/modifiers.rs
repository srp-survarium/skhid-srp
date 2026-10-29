//! The wiki's modifiers, `/wiki/modifiers` on the 0.26g site: every kind of
//! modification an item can roll or comes with, its strengths and the items
//! that take it. Made from the server's own game data
//! ([`survarium::v026g`]), the tables and the roll rule
//! ([`Modification::rolls`]) the lobby gives modifications by, so the page
//! says what drops, whatever a release changes.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::item_params;
use survarium::v026g::localization::{self, Language};
use survarium::v026g::modifications::{self, Modification, Modifications};
use survarium::v026g::ui_properties::{Change, UiProperties, UiProperty};

use crate::{Lang, escape};

/// The lobby's kinds of item that take modifications: its shop filters,
/// weapons first, then the kit slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    AssaultRifle,
    SubmachineGun,
    MachineGun,
    SniperRifle,
    Shotgun,
    Pistol,
    Helmet,
    Mask,
    Jacket,
    Back,
    Gloves,
    Pants,
    Boots,
}

impl Group {
    /// The group of an item of `category` (`items_dict`'s `item_category`).
    fn of(category: u8) -> Option<Self> {
        Some(match category {
            8 => Self::AssaultRifle,
            15 => Self::SubmachineGun,
            16 => Self::MachineGun,
            13 => Self::SniperRifle,
            14 => Self::Shotgun,
            17 => Self::Pistol,
            1 => Self::Helmet,
            2 => Self::Mask,
            3 => Self::Jacket,
            4 => Self::Back,
            6 => Self::Gloves,
            5 => Self::Pants,
            7 => Self::Boots,
            _ => return None,
        })
    }

    fn is_weapon(self) -> bool {
        self <= Self::Pistol
    }

    /// The localization key of the lobby's name for it.
    fn name_key(self) -> &'static str {
        match self {
            Self::AssaultRifle => "st_filter_assault_rifle",
            Self::SubmachineGun => "st_filter_submachine_gun",
            Self::MachineGun => "st_filter_machine_gun",
            Self::SniperRifle => "st_filter_sniper_rifle",
            Self::Shotgun => "st_filter_shotgun",
            Self::Pistol => "st_filter_pistol",
            Self::Helmet => "st_filter_helmet",
            Self::Mask => "st_filter_mask",
            Self::Jacket => "st_filter_body_armor",
            Self::Back => "st_backpack",
            Self::Gloves => "st_filter_gloves",
            Self::Pants => "st_filter_pants",
            Self::Boots => "st_filter_boots",
        }
    }
}

/// The items of a group that take a kind, of all the group's.
struct Fit {
    group: Group,
    /// By level, then dictionary order.
    items: Vec<u16>,
    of: usize,
}

/// How an item gets a kind: by a roll (a drop, an upgrade, a reroll), or
/// only built into some items (`default_modifications`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Rolled,
    BuiltIn,
}

/// One kind of modification (`type_mask`): what the lobby prints for it,
/// at each strength, and the items that take it.
struct Kind {
    property: &'static UiProperty,
    /// The values the lobby prints, the commonest first.
    strengths: Vec<f32>,
    source: Source,
    fits: Vec<Fit>,
}

impl Kind {
    fn for_weapons(&self) -> bool {
        self.fits.iter().all(|fit| fit.group.is_weapon())
    }
}

/// Every kind an item takes, in the game's order.
fn kinds() -> &'static [Kind] {
    static KINDS: OnceLock<Vec<Kind>> = OnceLock::new();
    KINDS.get_or_init(|| {
        let data = Modifications::shared();
        let properties = UiProperties::shared();
        let dictionary = Dictionary::shared();
        let mut by_kind: BTreeMap<u8, Vec<&Modification>> = BTreeMap::new();
        for modification in &data.modifications {
            by_kind
                .entry(modification.kind)
                .or_default()
                .push(modification);
        }
        let mut group_size: BTreeMap<Group, usize> = BTreeMap::new();
        let mut items: Vec<(Group, &item_params::ItemParams)> = Vec::new();
        for item in &dictionary.items {
            if let (Some(group), Some(params)) = (
                Group::of(item.category),
                item_params::item_params(item.dict_id),
            ) {
                *group_size.entry(group).or_default() += 1;
                items.push((group, params));
            }
        }
        items.sort_by_key(|&(group, params)| (group, params.item_level, params.dict_id));
        by_kind
            .into_values()
            .filter_map(|mut rows| {
                // Reliability: items wear nothing, so it neither rolls nor counts.
                if rows.iter().all(|row| row.only_changes_wear()) {
                    return None;
                }
                let rolled: Vec<&Modification> =
                    rows.iter().copied().filter(|row| row.rolls()).collect();
                let source = if rolled.is_empty() {
                    Source::BuiltIn
                } else {
                    rows = rolled;
                    Source::Rolled
                };
                rows.sort_by(|a, b| {
                    b.drop_weight
                        .total_cmp(&a.drop_weight)
                        .then(a.id.cmp(&b.id))
                });
                let kind = rows[0].kind;
                let ids: Vec<u32> = rows.iter().map(|row| row.id).collect();
                let takes = |params: &item_params::ItemParams| match source {
                    Source::Rolled => modifications::item_mods(params.dict_id)
                        .is_some_and(|mods| mods.modifications & (1 << kind) != 0),
                    Source::BuiltIn => params
                        .default_modifications
                        .iter()
                        .any(|id| ids.contains(id)),
                };
                let mut fits: Vec<Fit> = Vec::new();
                for &(group, params) in &items {
                    if !takes(params) {
                        continue;
                    }
                    match fits.last_mut() {
                        Some(fit) if fit.group == group => fit.items.push(params.dict_id),
                        _ => fits.push(Fit {
                            group,
                            items: vec![params.dict_id],
                            of: group_size[&group],
                        }),
                    }
                }
                if fits.is_empty() {
                    return None;
                }
                Some(Kind {
                    property: properties
                        .get(rows[0].shown.property)
                        .expect("srp checked every modification's property at start"),
                    strengths: rows.iter().map(|row| row.shown.value).collect(),
                    source,
                    fits,
                })
            })
            .collect()
    })
}

fn language(lang: Lang) -> Language {
    match lang {
        Lang::En => Language::English,
        Lang::Uk => Language::Ukrainian,
    }
}

/// The game's text of `key`, else the key.
fn text(lang: Lang, key: &str) -> String {
    localization::text(language(lang), key)
        .unwrap_or(key)
        .trim()
        .to_owned()
}

/// A number as the lobby would print it: at most two decimals, no trailing
/// zeros.
fn number(value: f32) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// One strength as the lobby prints it: a signed percentage for a change
/// that multiplies, a signed amount with its unit for one that adds.
fn strength(lang: Lang, property: &UiProperty, value: f32) -> String {
    let sign = if value > 0.0 { "+" } else { "" };
    match property.change {
        Some(Change::Multiplies) => format!("{sign}{}%", number(value * 100.0)),
        Some(Change::Adds) | None => match &property.postfix {
            Some(unit) => format!("{sign}{} {}", number(value), text(lang, unit)),
            None => format!("{sign}{}", number(value)),
        },
    }
}

/// An item's name for the lists.
fn item_name(lang: Lang, dict_id: u16) -> String {
    let dictionary = Dictionary::shared();
    let item = dictionary.item(dict_id).expect("an item of the dictionary");
    let name = item_params::item_params(dict_id)
        .and_then(|params| params.name_key)
        .map_or_else(|| item.name.clone(), |key| text(lang, key));
    if item.is_premium {
        format!("{name} {}", lang.tr("(premium)", "(преміум)"))
    } else {
        name
    }
}

fn kind_html(lang: Lang, kind: &Kind) -> String {
    let name = text(
        lang,
        kind.property
            .name
            .as_deref()
            .expect("srp checked every modification's property names itself"),
    );
    let strengths: Vec<String> = kind
        .strengths
        .iter()
        .map(|&value| format!("<li>{}</li>", escape(&strength(lang, kind.property, value))))
        .collect();
    let fits: Vec<String> = kind
        .fits
        .iter()
        .map(|fit| {
            let group = escape(&text(lang, fit.group.name_key()));
            if fit.items.len() == fit.of {
                format!("<li class=fit>{group}</li>")
            } else {
                let names: Vec<String> = fit
                    .items
                    .iter()
                    .map(|&dict_id| format!("<li>{}</li>", escape(&item_name(lang, dict_id))))
                    .collect();
                format!(
                    "<li class=fit><details><summary>{group} <span>{}</span></summary>\
                     <ul class=fit-items>{}</ul></details></li>",
                    lang.tr(
                        &format!("{} of {}", fit.items.len(), fit.of),
                        &format!("{} з {}", fit.items.len(), fit.of),
                    ),
                    names.concat(),
                )
            }
        })
        .collect();
    format!(
        "<div class=modifier><h3>{}</h3><ol class=strengths>{}</ol><ul class=fits>{}</ul></div>",
        escape(&name),
        strengths.concat(),
        fits.concat(),
    )
}

/// The page's body in `lang`, under the wiki's breadcrumb.
pub(crate) fn body(lang: Lang) -> String {
    let code = lang.code();
    let section = |title: String, note: String, wanted: &dyn Fn(&Kind) -> bool| {
        let rows: Vec<String> = kinds()
            .iter()
            .filter(|kind| wanted(kind))
            .map(|kind| kind_html(lang, kind))
            .collect();
        format!(
            "<section class=modifiers><h2>{}</h2><p class=wiki-note>{}</p>{}</section>",
            escape(&title),
            escape(&note),
            rows.concat()
        )
    };
    format!(
        "<p class=breadcrumb><a href=\"/wiki?lang={code}\">{}</a> / {}</p>\
         <div class=wiki-heading><div><p class=eyebrow>{}</p><h1>{}</h1></div></div>\
         <p class=wiki-intro>{}</p>{}{}{}",
        lang.tr("Wiki", "Вікі"),
        lang.tr("Modifiers", "Модифікатори"),
        lang.tr("Equipment reference", "Довідник спорядження"),
        lang.tr("Modifiers", "Модифікатори"),
        lang.tr(
            "An item rolls up to three modifiers, each of a different kind. The stronger, the rarer.",
            "Предмет отримує до трьох модифікаторів, кожен іншого виду. Що сильніший, то рідший.",
        ),
        section(
            lang.tr("Weapons", "Зброя"),
            lang.tr("Strengths from common to rare.", "Сила від звичайної до рідкісної."),
            &|kind| kind.source == Source::Rolled && kind.for_weapons(),
        ),
        section(
            lang.tr("Equipment", "Спорядження"),
            lang.tr("Strengths from common to rare.", "Сила від звичайної до рідкісної."),
            &|kind| kind.source == Source::Rolled && !kind.for_weapons(),
        ),
        section(
            lang.tr("Built into some items", "Вбудовані в деякі предмети"),
            lang.tr("These never roll.", "Ці не випадають."),
            &|kind| kind.source == Source::BuiltIn,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kind the lobby prints as property `id`.
    fn kind_shown_as(id: u32) -> &'static Kind {
        kinds()
            .iter()
            .find(|kind| kind.property.id == id)
            .unwrap_or_else(|| panic!("property {id}"))
    }

    #[test]
    fn damage_rolls_on_every_weapon_from_one_to_five_percent() {
        let damage = kind_shown_as(1);
        assert_eq!(damage.source, Source::Rolled);
        assert_eq!(damage.strengths, [0.01, 0.02, 0.03, 0.04, 0.05]);
        assert!(damage.for_weapons());
        assert_eq!(damage.fits.len(), 6, "every weapon class");
        assert!(damage.fits.iter().all(|fit| fit.items.len() == fit.of));
        let page = kind_html(Lang::En, damage);
        assert!(page.contains("<li>+1%</li>") && page.contains("<li>+5%</li>"));
    }

    #[test]
    fn a_kind_some_items_of_a_group_take_lists_them() {
        // The rate of fire: automatic weapons only.
        let rate = kind_shown_as(4);
        let sniper = rate
            .fits
            .iter()
            .find(|fit| fit.group == Group::SniperRifle)
            .unwrap();
        assert!(sniper.items.len() < sniper.of);
        let page = kind_html(Lang::En, rate);
        assert!(page.contains(&format!("{} of {}", sniper.items.len(), sniper.of)));
    }

    #[test]
    fn what_never_rolls_is_built_in_and_reliability_is_left_out() {
        assert!(kinds().iter().all(|kind| kind.property.change.is_some()));
        // Pocket and artefact slots come with some items only.
        assert!(
            kinds()
                .iter()
                .any(|kind| kind.source == Source::BuiltIn && !kind.for_weapons())
        );
        assert!(kinds().iter().all(|kind| !kind.fits.is_empty()));
    }

    #[test]
    fn the_strengths_print_as_the_lobby_does() {
        let weight_limit = kind_shown_as(106);
        let kg = text(Lang::En, weight_limit.property.postfix.as_deref().unwrap());
        assert_eq!(
            strength(Lang::En, weight_limit.property, 2.0),
            format!("+2 {kg}")
        );
        let damage = kind_shown_as(1);
        assert_eq!(strength(Lang::En, damage.property, -0.15), "-15%");
    }

    #[test]
    fn both_languages_have_the_three_sections() {
        for lang in [Lang::En, Lang::Uk] {
            let page = body(lang);
            assert_eq!(page.matches("<section class=modifiers>").count(), 3);
            assert!(page.contains(&format!("href=\"/wiki?lang={}\"", lang.code())));
        }
    }
}

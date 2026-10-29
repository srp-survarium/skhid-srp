//! The wiki's modifiers, `/wiki/modifiers` on the 0.26g site: every kind of
//! modification an item can roll or comes with, its strengths and the items
//! that take it; and, for the item `?item=` names, what it can roll and what
//! it comes with. Made from the server's own game data
//! ([`survarium::v026g`]), the tables and the roll rule
//! ([`Modification::rolls`]) the lobby gives modifications by, so the page
//! says what drops, whatever a release changes. The icons are the lobby's
//! ([`crate::icons`]).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use survarium::v026g::dictionary::{Dictionary, Item};
use survarium::v026g::item_params::{self, ItemParams};
use survarium::v026g::localization::{self, Language};
use survarium::v026g::modifications::{self, Modification, Modifications};
use survarium::v026g::ui_properties::{Change, UiProperties, UiProperty};

use crate::icons::{self, IconSet};
use crate::{Lang, escape};

/// The page's script: the weapons page's chooser and the page's own part,
/// built by `srp-026g/scripts/wiki/build.py`.
const SCRIPT: &[u8] = include_bytes!("../wiki/modifiers.js");
pub(crate) const SCRIPT_PATH: &str = "/wiki/modifiers.js";

/// The page's script, by [`SCRIPT_PATH`].
pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    (path == SCRIPT_PATH).then_some(("text/javascript; charset=utf-8", SCRIPT))
}

/// The script's address with its version, so it can be kept for good.
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

    /// Its `item_category`, which names its icon.
    fn category(self) -> u8 {
        match self {
            Self::AssaultRifle => 8,
            Self::SubmachineGun => 15,
            Self::MachineGun => 16,
            Self::SniperRifle => 13,
            Self::Shotgun => 14,
            Self::Pistol => 17,
            Self::Helmet => 1,
            Self::Mask => 2,
            Self::Jacket => 3,
            Self::Back => 4,
            Self::Gloves => 6,
            Self::Pants => 5,
            Self::Boots => 7,
        }
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
    id: u8,
    property: &'static UiProperty,
    /// Its `ui_desc.icon`.
    icon: u32,
    /// The values the lobby prints, the commonest first.
    strengths: Vec<f32>,
    source: Source,
    fits: Vec<Fit>,
}

impl Kind {
    fn for_weapons(&self) -> bool {
        self.fits.iter().all(|fit| fit.group.is_weapon())
    }

    fn takes(&self, dict_id: u16) -> bool {
        self.fits.iter().any(|fit| fit.items.contains(&dict_id))
    }
}

/// An item that takes a kind, with its group: what the chooser offers.
struct Choice {
    group: Group,
    item: &'static Item,
    params: &'static ItemParams,
}

/// Every item of a group, by group, level and dictionary order.
fn grouped() -> &'static [Choice] {
    static ITEMS: OnceLock<Vec<Choice>> = OnceLock::new();
    ITEMS.get_or_init(|| {
        let mut items: Vec<Choice> = Dictionary::shared()
            .items
            .iter()
            .filter_map(|item| {
                Some(Choice {
                    group: Group::of(item.category)?,
                    item,
                    params: item_params::item_params(item.dict_id)?,
                })
            })
            .collect();
        items.sort_by_key(|choice| (choice.group, choice.params.item_level, choice.item.dict_id));
        items
    })
}

/// Every kind an item takes, in the game's order.
fn kinds() -> &'static [Kind] {
    static KINDS: OnceLock<Vec<Kind>> = OnceLock::new();
    KINDS.get_or_init(|| {
        let properties = UiProperties::shared();
        let mut by_kind: BTreeMap<u8, Vec<&Modification>> = BTreeMap::new();
        for modification in &Modifications::shared().modifications {
            by_kind
                .entry(modification.kind)
                .or_default()
                .push(modification);
        }
        let mut group_size: BTreeMap<Group, usize> = BTreeMap::new();
        for choice in grouped() {
            *group_size.entry(choice.group).or_default() += 1;
        }
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
                let takes = |params: &ItemParams| match source {
                    Source::Rolled => modifications::item_mods(params.dict_id)
                        .is_some_and(|mods| mods.modifications & (1 << kind) != 0),
                    Source::BuiltIn => params
                        .default_modifications
                        .iter()
                        .any(|id| ids.contains(id)),
                };
                let mut fits: Vec<Fit> = Vec::new();
                for choice in grouped() {
                    if !takes(choice.params) {
                        continue;
                    }
                    match fits.last_mut() {
                        Some(fit) if fit.group == choice.group => {
                            fit.items.push(choice.item.dict_id);
                        }
                        _ => fits.push(Fit {
                            group: choice.group,
                            items: vec![choice.item.dict_id],
                            of: group_size[&choice.group],
                        }),
                    }
                }
                if fits.is_empty() {
                    return None;
                }
                Some(Kind {
                    id: kind,
                    property: properties
                        .get(rows[0].shown.property)
                        .expect("srp checked every modification's property at start"),
                    icon: rows[0].shown.icon,
                    strengths: rows.iter().map(|row| row.shown.value).collect(),
                    source,
                    fits,
                })
            })
            .collect()
    })
}

/// The items the chooser offers: those that take a kind.
fn choices() -> impl Iterator<Item = &'static Choice> {
    grouped()
        .iter()
        .filter(|choice| kinds().iter().any(|kind| kind.takes(choice.item.dict_id)))
}

/// The item `query` (a request's text after `?`) names by `item=`, its
/// `items_dict` name, if the chooser offers it.
pub(crate) fn chosen(query: &str) -> Option<u16> {
    let name = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("item="))?;
    choices()
        .find(|choice| choice.item.name == name)
        .map(|choice| choice.item.dict_id)
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

/// The name the lobby prints for `property`.
fn property_name(lang: Lang, property: &UiProperty) -> String {
    text(
        lang,
        property
            .name
            .as_deref()
            .expect("srp checked every modification's property names itself"),
    )
}

/// An item's name for the lists.
fn item_name(lang: Lang, dict_id: u16) -> String {
    let item = Dictionary::shared()
        .item(dict_id)
        .expect("an item of the dictionary");
    let name = item_params::item_params(dict_id)
        .and_then(|params| params.name_key)
        .map_or_else(|| item.name.clone(), |key| text(lang, key));
    if item.is_premium {
        format!("{name} {}", lang.tr("(premium)", "(преміум)"))
    } else {
        name
    }
}

/// A lobby glyph drawn in the text's colour (a CSS mask), or nothing if the
/// build made no icon for it.
fn glyph(set: IconSet, key: &str) -> String {
    icons::url(set, key).map_or_else(String::new, |url| {
        format!(
            "<span class=glyph style=\"--icon:url('{}')\" aria-hidden=true></span>",
            escape(&url)
        )
    })
}

/// An item's lobby picture, or nothing.
fn picture(dict_id: u16) -> String {
    let item = Dictionary::shared()
        .item(dict_id)
        .expect("an item of the dictionary");
    icons::url(IconSet::Item, &item.name).map_or_else(String::new, |url| {
        format!(
            "<img src=\"{}\" alt=\"\" width=86 height=42 loading=lazy>",
            escape(&url)
        )
    })
}

/// A group's icon with its name for the reader and as a tip; its name as
/// text if the build made no icon.
fn group_label(lang: Lang, group: Group) -> String {
    let name = escape(&text(lang, group.name_key()));
    match icons::url(IconSet::Group, &group.category().to_string()) {
        Some(_) => format!(
            "<span class=group title=\"{name}\">{}<span class=sr>{name}</span></span>",
            glyph(IconSet::Group, &group.category().to_string())
        ),
        None => format!("<span class=group>{name}</span>"),
    }
}

/// A kind's icon and name.
fn kind_title(lang: Lang, kind: &Kind) -> String {
    format!(
        "{}<span>{}</span>",
        glyph(IconSet::Modifier, &kind.icon.to_string()),
        escape(&property_name(lang, kind.property))
    )
}

fn strengths_html(lang: Lang, kind: &Kind) -> String {
    let strengths: Vec<String> = kind
        .strengths
        .iter()
        .map(|&value| format!("<li>{}</li>", escape(&strength(lang, kind.property, value))))
        .collect();
    format!("<ol class=strengths>{}</ol>", strengths.concat())
}

/// An item of a list: its picture and name, a link to its own modifiers.
fn item_link(lang: Lang, dict_id: u16) -> String {
    let item = Dictionary::shared()
        .item(dict_id)
        .expect("an item of the dictionary");
    format!(
        "<li><a href=\"?lang={}&amp;item={}#item\">{}<span>{}</span></a></li>",
        lang.code(),
        escape(&item.name),
        picture(dict_id),
        escape(&item_name(lang, dict_id))
    )
}

fn kind_html(lang: Lang, kind: &Kind) -> String {
    let fits: Vec<String> = kind
        .fits
        .iter()
        .map(|fit| {
            let group = group_label(lang, fit.group);
            if fit.items.len() == fit.of {
                format!("<li class=fit>{group}</li>")
            } else {
                let items: Vec<String> = fit
                    .items
                    .iter()
                    .map(|&dict_id| item_link(lang, dict_id))
                    .collect();
                format!(
                    "<li class=fit><details><summary>{group} <span class=count>{}</span></summary>\
                     <ul class=fit-items>{}</ul></details></li>",
                    lang.tr(
                        &format!("{} of {}", fit.items.len(), fit.of),
                        &format!("{} з {}", fit.items.len(), fit.of),
                    ),
                    items.concat(),
                )
            }
        })
        .collect();
    format!(
        "<div class=modifier id=kind-{}><h3>{}</h3>{}<ul class=fits>{}</ul></div>",
        kind.id,
        kind_title(lang, kind),
        strengths_html(lang, kind),
        fits.concat(),
    )
}

/// The chooser and, for `item`, what it can roll and what it comes with.
fn item_html(lang: Lang, item: Option<u16>) -> String {
    let options: Vec<String> = choices()
        .map(|choice| {
            let dict_id = choice.item.dict_id;
            format!(
                "<div id=choice-{dict_id} data-key=\"{}\" data-class=\"{}\" data-level={} \
                 data-title=\"{}\" data-search-name=\"{}\">{}</div>",
                escape(&choice.item.name),
                escape(&text(lang, choice.group.name_key())),
                choice.params.item_level,
                escape(&item_name(lang, dict_id)),
                escape(&choice.item.name),
                picture(dict_id),
            )
        })
        .collect();
    let select_options: Vec<String> = choices()
        .map(|choice| {
            let dict_id = choice.item.dict_id;
            format!(
                "<option value=choice-{dict_id}{}>{}</option>",
                if item == Some(dict_id) {
                    " selected"
                } else {
                    ""
                },
                escape(&item_name(lang, dict_id))
            )
        })
        .collect();
    let chooser = format!(
        "<div class=compare-field><label id=item-choice-label for=item-choice-btn>{}</label>\
         <div class=wpick id=item-choice-picker></div><select id=item-choice hidden>\
         <option value=\"\">{}</option>{}</select></div><div id=item-options hidden>{}</div>",
        lang.tr("Item", "Предмет"),
        lang.tr("Choose an item…", "Виберіть предмет…"),
        select_options.concat(),
        options.concat(),
    );
    let Some(dict_id) = item else {
        return format!(
            "<section class=item-modifiers id=item><h2>{}</h2>{chooser}</section>",
            lang.tr("By item", "За предметом")
        );
    };
    let params = item_params::item_params(dict_id).expect("an offered item has its parameters");
    let rolls: Vec<String> = kinds()
        .iter()
        .filter(|kind| kind.source == Source::Rolled && kind.takes(dict_id))
        .map(|kind| {
            format!(
                "<li><a class=kind href=\"#kind-{}\">{}</a>{}</li>",
                kind.id,
                kind_title(lang, kind),
                strengths_html(lang, kind)
            )
        })
        .collect();
    let properties = UiProperties::shared();
    let built_in: Vec<String> = params
        .default_modifications
        .iter()
        .filter_map(|&id| Modifications::shared().modification(id))
        .filter(|modification| !modification.only_changes_wear())
        .filter_map(|modification| {
            let property = properties.get(modification.shown.property)?;
            Some(format!(
                "<li><span class=kind>{}<span>{}</span></span><ol class=strengths><li>{}</li></ol></li>",
                glyph(IconSet::Modifier, &modification.shown.icon.to_string()),
                escape(&property_name(lang, property)),
                escape(&strength(lang, property, modification.shown.value)),
            ))
        })
        .collect();
    let list = |title: String, rows: Vec<String>| {
        if rows.is_empty() {
            String::new()
        } else {
            format!(
                "<h3>{}</h3><ul class=item-kinds>{}</ul>",
                escape(&title),
                rows.concat()
            )
        }
    };
    format!(
        "<section class=item-modifiers id=item><h2>{}</h2>{chooser}\
         <div class=item-card><div class=item-head>{}<div><strong>{}</strong><span>{} · {}</span></div></div>{}{}</div></section>",
        lang.tr("By item", "За предметом"),
        picture(dict_id),
        escape(&item_name(lang, dict_id)),
        group_label(
            lang,
            Group::of(
                Dictionary::shared()
                    .item(dict_id)
                    .expect("an item")
                    .category
            )
            .expect("an offered item has a group")
        ),
        lang.tr(
            &format!("level {}", params.item_level),
            &format!("рівень {}", params.item_level)
        ),
        list(lang.tr("Can roll", "Може отримати"), rolls),
        list(lang.tr("Comes with", "Має від початку"), built_in),
    )
}

/// The page's body in `lang`, under the wiki's breadcrumb, with `item`'s
/// modifiers if one is chosen.
pub(crate) fn body(lang: Lang, item: Option<u16>) -> String {
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
         <p class=wiki-intro>{}</p>{}{}{}{}<script src=\"{}\"></script>",
        lang.tr("Wiki", "Вікі"),
        lang.tr("Modifiers", "Модифікатори"),
        lang.tr("Equipment reference", "Довідник спорядження"),
        lang.tr("Modifiers", "Модифікатори"),
        lang.tr(
            "An item rolls up to three modifiers, each of a different kind. The stronger, the rarer.",
            "Предмет отримує до трьох модифікаторів, кожен іншого виду. Що сильніший, то рідший.",
        ),
        item_html(lang, item),
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
        script_url(),
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

    fn dict_id(name: &str) -> u16 {
        Dictionary::shared().item_named(name).unwrap().dict_id
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
        // Each listed item links to its own modifiers.
        let first = Dictionary::shared().item(sniper.items[0]).unwrap();
        assert!(page.contains(&format!("href=\"?lang=en&amp;item={}#item\"", first.name)));
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
    fn an_item_is_chosen_by_its_dictionary_name_only() {
        let ak = dict_id("ak_74m");
        assert_eq!(chosen("lang=en&item=ak_74m"), Some(ak));
        assert_eq!(chosen("item=ak_74m"), Some(ak));
        assert_eq!(chosen("item=no_such_item"), None);
        assert_eq!(chosen("lang=uk"), None);
        // An item that takes nothing (ammunition) is not offered.
        let ammo = Dictionary::shared()
            .items
            .iter()
            .find(|item| item.name.starts_with("ammo_"))
            .unwrap();
        assert_eq!(chosen(&format!("item={}", ammo.name)), None);
    }

    #[test]
    fn a_chosen_weapon_lists_what_it_rolls() {
        let page = item_html(Lang::En, Some(dict_id("ak_74m")));
        assert!(page.contains("<div class=item-card>"));
        assert!(page.contains("Can roll"));
        // Damage rolls on it; chambering (bolt-action only) does not.
        assert!(page.contains(&format!("href=\"#kind-{}\"", kind_shown_as(1).id)));
        assert!(!page.contains(&format!("href=\"#kind-{}\"", kind_shown_as(119).id)));
        assert!(page.contains("<option value=choice-") && page.contains(" selected>"));
    }

    #[test]
    fn both_languages_have_the_chooser_and_the_three_sections() {
        for lang in [Lang::En, Lang::Uk] {
            let page = body(lang, None);
            assert_eq!(page.matches("<section class=modifiers>").count(), 3);
            assert!(page.contains("id=item-choice"));
            assert!(!page.contains("<div class=item-card>"));
            assert!(page.contains(&format!("href=\"/wiki?lang={}\"", lang.code())));
            assert!(page.contains(&format!("<script src=\"{}\"></script>", script_url())));
        }
    }
}

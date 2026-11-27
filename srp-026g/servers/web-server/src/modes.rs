//! The wiki's modes, `/wiki/modes` on the 0.26g site: every mode a match can
//! be, and a free-for-all's versions (twists), on one page, a section each
//! with its anchor (`#team-deathmatch`, `#racket`). Each has its
//! loading-screen icon ([`IconSet::Mode`]), its rules in short lines, the
//! numbers they hold, and whether the
//! rooms offer it. Nothing is typed in that the server has:
//! - the numbers (time limit, respawn, batteries, chargers,
//!   tickets, points, kills, Artifact Hunt's rule) are the playable configs'
//!   ([`maps::playable_in`], [`maps::MapMode`]): a span where maps differ;
//! - Freeze Tag's are its rule's ([`freeze`]), a free-for-all's versions
//!   their shares and arms ([`twist::SHARES`], [`racket`], [`cowboy`],
//!   [`roulette`], [`pistols`]);
//! - the rooms' offer is the running lobby's pools ([`Pools`]).

use std::collections::BTreeSet;
use std::fmt::Write;

use srp_validate::world::freeze;
use survarium::v026g::cowboy::{self, Variation};
use survarium::v026g::dictionary::{Dictionary, MapId};
use survarium::v026g::item_params;
use survarium::v026g::maps::{self, GameMode, MapMode};
use survarium::v026g::pistols;
use survarium::v026g::racket::{self, Arms, Gun};
use survarium::v026g::roulette::{self, Band};
use survarium::v026g::twist::{self, LobbyMode, Pick};

use crate::icons::{self, IconSet};
use crate::launcher::lobby_mode_name;
use crate::{Lang, escape, modifiers};

/// The page's address.
pub(crate) const PATH: &str = "/wiki/modes";

/// Where the rooms offer the maps: the lobby's pools as the server runs
/// them (`SRP_MATCH_MAPS` and `SRP_ROOMS_EXTRA_MAPS`; the lobby's
/// `Core::room_entries` and `private_entries`).
#[derive(Clone, Debug)]
pub struct Pools {
    /// The match pool: every public room's, and a private room's too.
    public: BTreeSet<MapId>,
    /// What a private room's maker may add besides (the experimental maps),
    /// none of them in the match pool.
    experimental: BTreeSet<MapId>,
    /// Both, in the order the rooms' create window lists them (the lobby's
    /// `room_entries`: [`maps::in_room_order`] of the match pool, then the
    /// experimental maps): the stats' Maps board's order.
    room_order: Vec<MapId>,
}

/// Where a map is offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Offer {
    /// In the match pool.
    Public,
    /// An experimental map: a private room's maker adds it.
    PrivateOnly,
}

impl Pools {
    /// The lobby's match pool and its experimental maps.
    pub fn new(public_ids: &[MapId], experimental: &[MapId]) -> Self {
        let public: BTreeSet<MapId> = public_ids.iter().copied().collect();
        let experimental: Vec<MapId> = experimental
            .iter()
            .copied()
            .filter(|id| !public.contains(id))
            .collect();
        // The pool in its own order, then the experimental maps, as the lobby takes them.
        let listed: Vec<&MapMode> = public_ids
            .iter()
            .chain(&experimental)
            .filter_map(|id| maps::map_mode(*id))
            .collect();
        let room_order = maps::in_room_order(listed, |map| (map.build, map.level, map.weather))
            .into_iter()
            .map(|map| map.id)
            .collect();
        Self {
            public,
            experimental: experimental.into_iter().collect(),
            room_order,
        }
    }

    /// The maps the rooms' create window lists, in its order.
    pub(crate) fn room_order(&self) -> &[MapId] {
        &self.room_order
    }

    /// What the lobby plays with neither set: every playable map in the
    /// match pool.
    pub fn every_playable() -> Self {
        let ids: Vec<MapId> = maps::playable().map(|map| map.id).collect();
        Self::new(&ids, &[])
    }

    /// Where `map` is offered: a map played with a free-for-all's versions
    /// only is never a private room's (`private_entries`).
    fn offer(&self, map: &MapMode) -> Option<Offer> {
        if self.public.contains(&map.id) {
            Some(Offer::Public)
        } else if self.experimental.contains(&map.id) && !map.twists_only {
            Some(Offer::PrivateOnly)
        } else {
            None
        }
    }
}

/// The rooms a mode is played in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rooms {
    Both,
    PublicOnly,
    PrivateOnly,
    None,
}

impl Rooms {
    /// A mode's, from its maps: public with a map of the match pool, private
    /// with one a private room may hold.
    fn of(pools: &Pools, maps: &[&MapMode]) -> Self {
        let offers: Vec<(Offer, bool)> = maps
            .iter()
            .filter_map(|map| pools.offer(map).map(|offer| (offer, map.twists_only)))
            .collect();
        let public = offers.iter().any(|&(offer, _)| offer == Offer::Public);
        let private = offers.iter().any(|&(_, twists_only)| !twists_only);
        match (public, private) {
            (true, true) => Self::Both,
            (true, false) => Self::PublicOnly,
            (false, true) => Self::PrivateOnly,
            (false, false) => Self::None,
        }
    }

    /// The line under a mode's title: none when every room offers it.
    fn line(self, lang: Lang) -> String {
        let text = match self {
            Self::Both => return String::new(),
            Self::PublicOnly => lang.t("Public rooms only"),
            Self::PrivateOnly => lang.t("Private rooms only"),
            Self::None => lang.t("Not offered now"),
        };
        format!("<p class=mode-rooms>{text}</p>")
    }
}

/// A mode's section on the Modes page, in `lang`.
pub(crate) fn url(lang: Lang, mode: LobbyMode) -> String {
    format!("{PATH}?lang={}#{}", lang.code(), anchor(mode))
}

/// A section's anchor: the lobby mode's.
fn anchor(mode: LobbyMode) -> &'static str {
    match mode {
        LobbyMode::Plain(GameMode::Batteries) => "protective-device",
        LobbyMode::Plain(GameMode::TimedBatteries) => "chargers",
        LobbyMode::Plain(GameMode::Blackout) => "blackout",
        LobbyMode::Plain(GameMode::TeamDeathmatch) => "team-deathmatch",
        LobbyMode::Plain(GameMode::Research) => "research",
        LobbyMode::Plain(GameMode::FreeForAll) => "free-for-all",
        LobbyMode::Plain(GameMode::FreezeTag) => "freeze-tag",
        LobbyMode::Plain(GameMode::ArtefactHunt) => "artifact-hunt",
        LobbyMode::Racket => "racket",
        LobbyMode::Cowboy(Variation::Colt) => "saloon",
        LobbyMode::Cowboy(Variation::WinchesterAndColt) => "saloon-winchester",
        LobbyMode::Roulette(_) => "roulette",
        LobbyMode::Pistols => "pistols",
    }
}

/// The plain free-for-all's plaque among its versions: its own anchor, the mode's
/// section having the mode's.
const PLAIN_VERSION: &str = "plain";

/// The modes in the order the lobby lists them: the protective device's
/// kinds, then the rest.
const MODES: [GameMode; 8] = [
    GameMode::Batteries,
    GameMode::TimedBatteries,
    GameMode::Blackout,
    GameMode::TeamDeathmatch,
    GameMode::Research,
    GameMode::FreeForAll,
    GameMode::FreezeTag,
    GameMode::ArtefactHunt,
];

/// Distinct values as one: `a`, or `least–most`.
fn span<T: Ord + Copy>(values: impl IntoIterator<Item = T>, show: impl Fn(T) -> String) -> String {
    let values: BTreeSet<T> = values.into_iter().collect();
    match (values.first(), values.last()) {
        (Some(&least), Some(&most)) if least != most => format!("{}–{}", show(least), show(most)),
        (Some(&value), _) => show(value),
        _ => "—".to_owned(),
    }
}

/// Seconds as a clock: `15:00`.
fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Milliseconds as seconds: `3`, `2.5`.
fn seconds(ms: u32) -> String {
    modifiers::number(ms as f32 / 1000.0)
}

/// A mode's icon, as its maps' loading screen shows it: frame `ui.icon` + 1.
fn mode_icon(maps: &[&MapMode]) -> String {
    maps.first()
        .and_then(|map| icons::url(IconSet::Mode, &(u16::from(map.icon) + 1).to_string()))
        .map_or_else(String::new, |url| {
            format!("<img class=mode-icon src=\"{}\" alt=\"\">", escape(&url))
        })
}

/// A version's icon: the one the loading screen shows for it; the plain
/// free-for-all's its maps' ([`mode_icon`]).
fn version_icon(mode: LobbyMode) -> String {
    match mode {
        LobbyMode::Plain(mode) => mode_icon(&maps::playable_in(mode).collect::<Vec<_>>()),
        twist => icons::url(IconSet::Mode, &format!("twist-{}", twist.number())).map_or_else(String::new, |url| {
            format!("<img class=mode-icon src=\"{}\" alt=\"\">", escape(&url))
        }),
    }
}

/// A list of rules, a line each.
fn rules(lines: &[String]) -> String {
    let items: String = lines.iter().map(|line| format!("<li>{line}</li>")).collect();
    format!("<ul class=mode-rules>{items}</ul>")
}

/// A mode's rules in `lang`, its numbers from `maps`.
fn mode_rules(lang: Lang, mode: GameMode, maps: &[&MapMode]) -> Vec<String> {
    let number = |field: fn(&MapMode) -> u32| span(maps.iter().map(|map| field(map)), |n| n.to_string());
    let time = span(maps.iter().map(|map| map.match_time), clock);
    let respawn = number(|map| map.respawn_time);
    let batteries = number(|map| map.victory_items_count);
    let respawn_line = lang.tf("Respawn after {0} s.", &[("0", &respawn)]);
    match mode {
        GameMode::Batteries => vec![
            lang.t("Batteries lie on the map."),
            lang.t("Carry them to your team's container."),
            lang.tf("First to {0} batteries wins.", &[("0", &batteries)]),
            lang.tf("Time limit {0}: more batteries wins.", &[("0", &time)]),
            respawn_line,
        ],
        GameMode::TimedBatteries => vec![
            lang.tf(
                "Batteries come out of {0} active chargers.",
                &[("0", &number(|map| map.spawn_points_count))],
            ),
            lang.tf(
                "A new battery every {0} s, give or take {1} s.",
                &[
                    ("0", &number(|map| map.vi_spawn_time)),
                    ("1", &number(|map| map.vi_spawn_variation)),
                ],
            ),
            lang.t("Carry them to your team's container."),
            lang.tf("First to {0} batteries wins.", &[("0", &batteries)]),
            lang.tf("Time limit {0}: more batteries wins.", &[("0", &time)]),
            respawn_line,
        ],
        GameMode::Blackout => vec![
            lang.t("Protective device rules: batteries, no chargers."),
            lang.t("No interface: no ammo, health, crosshair or minimap."),
            lang.t("Teammates' markers stay; enemies have none."),
            lang.t("Hold Tab for ammo, body state, the clock and the team list."),
            lang.tf("First to {0} batteries wins.", &[("0", &batteries)]),
            lang.tf("Time limit {0}: more batteries wins.", &[("0", &time)]),
            respawn_line,
        ],
        GameMode::TeamDeathmatch => vec![
            lang.tf(
                "Each team has {0} tickets.",
                &[("0", &number(|map| map.tickets_count))],
            ),
            lang.t("A death costs one ticket; a teamkill costs none."),
            lang.t("Out of tickets, a team stops respawning and loses when its last player dies."),
            lang.tf("Time limit {0}: more tickets wins.", &[("0", &time)]),
            respawn_line,
        ],
        GameMode::Research => vec![
            lang.t("Capture research units: be inside with no enemy there."),
            lang.t("Owned units earn points; more units, faster."),
            lang.tf(
                "First to {0} points wins.",
                &[("0", &number(|map| map.resources_count))],
            ),
            lang.tf("Time limit {0}: more points wins.", &[("0", &time)]),
            respawn_line,
        ],
        GameMode::FreeForAll => vec![
            lang.t("Every player for himself."),
            lang.tf(
                "First to {0} kills wins.",
                &[("0", &number(|map| map.kills_to_win))],
            ),
            lang.tf("Time limit {0}: most kills wins.", &[("0", &time)]),
            respawn_line,
            lang.t("Public rooms: each FFA card draws a version."),
            lang.t("Private rooms: plain FFA only."),
        ],
        GameMode::FreezeTag => vec![
            lang.t("A lethal hit freezes you in place."),
            lang.tf(
                "A teammate within {0} m thaws you in {1} s.",
                &[
                    ("0", &modifiers::number(freeze::RADIUS)),
                    ("1", &seconds(u32::from(freeze::THAW_MS))),
                ],
            ),
            lang.t("A thaw counts as a kill."),
            lang.tf(
                "A frozen player thaws by himself after {0}.",
                &[("0", &clock(freeze::AUTO_THAW_MS.get() / 1000))],
            ),
            lang.t("A team's last fighter shows on the enemies' map."),
            lang.t("Freeze the whole enemy team to win the round."),
            lang.tf(
                "{0} s after a round ends, then everyone respawns and waits {1} s at the spawns.",
                &[
                    ("0", &seconds(freeze::TIMING.decided().get())),
                    ("1", &seconds(freeze::TIMING.countdown().get())),
                ],
            ),
            lang.tf("First to {0} rounds wins.", &[("0", &freeze::WINS)]),
            lang.tf(
                "Time limit {0}: an unfinished series is a draw.",
                &[("0", &time)],
            ),
        ],
        GameMode::ArtefactHunt => {
            let params: Vec<_> = maps.iter().filter_map(|map| map.artefact_hunt).collect();
            let goal = span(params.iter().map(|rule| rule.goal()), |n| n.to_string());
            let time = span(params.iter().map(|rule| rule.match_time().get() / 1000), clock);
            let sudden = span(params.iter().map(|rule| rule.sudden_death().get() / 1000), clock);
            let earliest = span(
                maps.iter().map(|map| map.vi_spawn_time.saturating_sub(map.vi_spawn_variation)),
                |n| n.to_string(),
            );
            let latest = span(
                maps.iter().map(|map| map.vi_spawn_time + map.vi_spawn_variation),
                |n| n.to_string(),
            );
            vec![
                lang.tf(
                    "One artefact at a time, {0}–{1} s into a round.",
                    &[("0", &earliest), ("1", &latest)],
                ),
                lang.t("Bring it to your own base: 1 point, and the round ends."),
                lang.t("The carrier is slower and seen by everyone."),
                lang.tf("Respawn in waves every {0} s.", &[("0", &respawn)]),
                lang.tf("First to {0} points wins.", &[("0", &goal)]),
                lang.tf(
                    "Time limit {0}: the higher score wins; a tie plays {1} of sudden death without respawns.",
                    &[("0", &time), ("1", &sudden)],
                ),
            ]
        }
    }
}

/// An item's name as the game shows it, without the lists' "(premium)":
/// what a version gives is the same gun whatever copy it is.
fn item_name(lang: Lang, dict_id: u16) -> String {
    let name = item_params::item_params(dict_id)
        .and_then(|params| params.name_key)
        .map_or_else(|| dict_id.to_string(), |key| modifiers::text(lang, key));
    latin_letters(&name)
}

/// `text` with each word that mixes Latin and Cyrillic letters (the stock
/// English names the AK-74N "АК-74N", its А and К Cyrillic) in Latin letters
/// alone, its Cyrillic letters by the Latin ones they look like. A word of
/// one script stays as it is, the Cyrillic languages' "АК-74Н" too.
fn latin_letters(text: &str) -> String {
    const LOOKALIKES: [(char, char); 18] = [
        ('А', 'A'), ('В', 'B'), ('Е', 'E'), ('К', 'K'), ('М', 'M'), ('Н', 'H'), ('О', 'O'), ('Р', 'P'), ('С', 'C'),
        ('Т', 'T'), ('Х', 'X'), ('а', 'a'), ('е', 'e'), ('о', 'o'), ('р', 'p'), ('с', 'c'), ('у', 'y'), ('х', 'x'),
    ];
    text.split(' ')
        .map(|word| {
            let cyrillic = word.chars().any(|c| ('\u{400}'..='\u{4FF}').contains(&c));
            let latin = word.chars().any(|c| c.is_ascii_alphabetic());
            if cyrillic && latin {
                word.chars()
                    .map(|c| LOOKALIKES.iter().find(|(from, _)| *from == c).map_or(c, |(_, to)| *to))
                    .collect()
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// An item's name by its dictionary name; `None` for one the data does not
/// have (the tests' data has no shop scopes), never its dictionary name.
fn named_item(lang: Lang, name: &str) -> Option<String> {
    Dictionary::shared()
        .item_named(name)
        .map(|item| item_name(lang, item.dict_id))
}

/// A free-for-all's version's rules.
fn version_rules(lang: Lang, mode: LobbyMode) -> Vec<String> {
    match mode {
        LobbyMode::Racket => {
            let gun = |gun: Gun| escape(&item_name(lang, Arms::shared(gun).gun()));
            let up_to = (1..=u8::MAX)
                .filter(|&level| Gun::of_room(level) == Gun::Ak74n)
                .max()
                .unwrap_or(0);
            let swapped = lang.tf(
                "Everyone fights with an assault rifle: sniper rifles, silenced guns and guns without automatic fire are swapped for an {0} (rooms up to level {1}) or an {2} (higher rooms).",
                &[("0", &gun(Gun::Ak74n)), ("1", &up_to), ("2", &gun(Gun::Ak12))],
            );
            // The sight, where the data has it.
            let sight = named_item(lang, racket::SIGHT)
                .map(|sight| lang.tf("Each comes with an {0}.", &[("0", &escape(&sight))]));
            std::iter::once(swapped).chain(sight).collect()
        }
        LobbyMode::Cowboy(Variation::Colt) => vec![lang.tf(
            "Every kit: {0} only.",
            &[("0", &escape(&named_item(lang, cowboy::COLT).unwrap_or_else(|| lang.t("a revolver"))))],
        )],
        LobbyMode::Cowboy(Variation::WinchesterAndColt) => vec![lang.tf(
            "Every kit: {0} and {1}.",
            &[
                ("0", &escape(&named_item(lang, cowboy::WINCHESTER).unwrap_or_else(|| lang.t("a lever-action rifle")))),
                ("1", &escape(&named_item(lang, cowboy::COLT).unwrap_or_else(|| lang.t("a revolver")))),
            ],
        )],
        LobbyMode::Roulette(_) => {
            let bands: Vec<String> = Band::ALL
                .iter()
                .map(|band| {
                    let (low, top) = band.levels();
                    format!("{low}–{top}")
                })
                .collect();
            vec![
                lang.tf("Everyone plays the same {0} random kits.", &[("0", &roulette::KITS)]),
                lang.tf("Level bands: {0}.", &[("0", &bands.join(", "))]),
                lang.t("A room draws bands up to its level."),
                lang.t("Each kit a gun of another class, with random bonuses and attachments."),
                lang.tf(
                    "{0} % of deals: pistols only.",
                    &[("0", &roulette::PISTOL_HANDS_PERCENT)],
                ),
            ]
        }
        LobbyMode::Pistols => {
            let mut lines = vec![lang.tf(
                "Everyone plays the same {0} random kits, each with one pistol; the third kit stays empty.",
                &[("0", &pistols::KITS)],
            )];
            // Each band's pistols, as the data has them.
            // 0-5 and 6-10 alone: an 11-15 room plays one of them (the owner, 2026-11-27).
            lines.extend(pistols::Band::ALL.iter().map(|&band| {
                let (low, top) = band.levels();
                let names: Vec<String> = pistols::pistols(band)
                    .into_iter()
                    .map(|gun| escape(&item_name(lang, gun)))
                    .collect();
                lang.tf(
                    "Pistols of levels {0}: {1}.",
                    &[("0", &format!("{low}–{top}")), ("1", &names.join(", "))],
                )
            }));
            lines.push(lang.t("A room draws bands up to its level; a room above level 10 plays 0–5 or 6–10."));
            lines.push(lang.t("Two different pistols of the band, with random armour, bonuses and attachments."));
            lines
        }
        LobbyMode::Plain(GameMode::FreeForAll) => vec![lang.t("Everyone plays his own kits.")],
        LobbyMode::Plain(_) => Vec::new(),
    }
}

/// The share of the free-for-all cards a version is drawn for, in percent.
fn share(pick: Pick) -> u32 {
    let total: u32 = twist::SHARES.iter().map(|&(_, share)| share).sum();
    twist::SHARES
        .iter()
        .filter(|&&(each, _)| each == pick)
        .map(|&(_, share)| share * 100 / total)
        .sum()
}

/// The free-for-all's versions: their subsections, the plain one's first
/// (the owner, 2026-11-27: the plain FFA had no plaque of its own).
fn versions(lang: Lang, out: &mut String) {
    let _ = write!(out, "<h3 class=mode-versions>{}</h3>", lang.t("Versions"));
    for (mode, pick) in [
        (LobbyMode::Plain(GameMode::FreeForAll), Pick::Plain),
        (LobbyMode::Racket, Pick::Racket),
        (LobbyMode::Cowboy(Variation::Colt), Pick::Cowboy(Variation::Colt)),
        (
            LobbyMode::Cowboy(Variation::WinchesterAndColt),
            Pick::Cowboy(Variation::WinchesterAndColt),
        ),
        (LobbyMode::Roulette(Band::Low), Pick::Roulette),
        (LobbyMode::Pistols, Pick::Pistols),
    ] {
        let name = match mode {
            LobbyMode::Roulette(_) => lang.t("FFA: Roulette"),
            other => lobby_mode_name(lang, other),
        };
        let mut lines = vec![lang.tf("{0} % of FFA cards.", &[("0", &share(pick))])];
        lines.extend(version_rules(lang, mode));
        let id = match mode {
            LobbyMode::Plain(_) => PLAIN_VERSION,
            twist => anchor(twist),
        };
        let _ = write!(
            out,
            "<section class=mode-version id={}><h3>{}{}</h3>{}</section>",
            id,
            version_icon(mode),
            escape(&name),
            rules(&lines),
        );
    }
}

/// The page's body in `lang`, for the wiki's page: the rooms' offer is
/// `pools`'.
pub(crate) fn body(lang: Lang, pools: &Pools) -> String {
    let code = lang.code();
    let mut index = String::from("<nav class=mode-index>");
    let mut sections = String::new();
    for mode in MODES {
        let all: Vec<&MapMode> = maps::playable_in(mode).collect();
        if all.is_empty() {
            continue;
        }
        let offered: Vec<&MapMode> = all
            .iter()
            .copied()
            .filter(|map| pools.offer(map).is_some())
            .collect();
        // A mode no room offers now: the maps it is played on when one does.
        let shown = if offered.is_empty() { &all } else { &offered };
        let rooms = Rooms::of(pools, &all);
        let lobby = LobbyMode::Plain(mode);
        let name = escape(&lobby_mode_name(lang, lobby));
        let icon = mode_icon(&all);
        let _ = write!(
            index,
            "<a href=\"#{}\">{icon}<span>{name}</span></a>",
            anchor(lobby)
        );
        let _ = write!(
            sections,
            "<section class=\"modifiers mode\" id={}><h2>{icon}{name}</h2>\
             {}<div class=mode-body>{}</div>",
            anchor(lobby),
            rooms.line(lang),
            rules(&mode_rules(lang, mode, shown)),
        );
        if mode == GameMode::FreeForAll {
            versions(lang, &mut sections);
        }
        sections.push_str("</section>");
    }
    index.push_str("</nav>");
    format!(
        "<p class=breadcrumb><a href=\"/wiki?lang={code}\">{}</a> / {}</p>\
         <div class=wiki-heading><div><h1>{}</h1></div></div>{index}{sections}",
        lang.t("Wiki"),
        lang.t("Modes"),
        lang.t("Modes"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether a word mixes Latin and Cyrillic letters.
    fn mixed(word: &str) -> bool {
        word.chars().any(|c| c.is_ascii_alphabetic()) && word.chars().any(|c| ('\u{400}'..='\u{4FF}').contains(&c))
    }

    /// No mode's or version's description shows a dictionary's name
    /// (`shop_scope_eotech_552`) or a word of two scripts ("АК-74N", the stock
    /// English name), in any language: the page and every version's rules.
    #[test]
    fn descriptions_name_items_in_words_of_one_script() {
        for lang in Lang::ALL {
            let page = body(lang, &Pools::every_playable());
            let mut texts: Vec<String> = LobbyMode::ALL.into_iter().flat_map(|mode| version_rules(lang, mode)).collect();
            texts.push(page);
            for text in &texts {
                for word in text.split(|c: char| c.is_whitespace() || "<>\"(),.:;".contains(c)) {
                    assert!(!(word.contains('_') && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')) || word.starts_with("data-") , "{}: a dictionary name {word:?} in {text:?}", lang.code());
                    assert!(!mixed(word), "{}: {word:?} mixes scripts", lang.code());
                }
            }
        }
        // The stock English AK-74N, in Latin letters; a Cyrillic language's own as it is.
        assert_eq!(latin_letters("АК-74N"), "AK-74N");
        assert_eq!(latin_letters("АК-74Н"), "АК-74Н", "a word of one script stays");
        assert_eq!(latin_letters("Набор AK"), "Набор AK");
    }

    /// The Racket's rules say what it is in a sentence, its sight where the
    /// data has it, never its dictionary name.
    #[test]
    fn the_rackets_rules_are_a_sentence() {
        let rules = version_rules(Lang::En, LobbyMode::Racket);
        assert!(rules[0].starts_with("Everyone fights with an assault rifle: "), "{rules:?}");
        assert!(rules[0].contains("(rooms up to level 10)") && rules[0].contains("(higher rooms)"), "{rules:?}");
        assert!(rules.iter().all(|rule| !rule.contains(racket::SIGHT)), "{rules:?}");
        assert_eq!(
            rules.len(),
            1 + usize::from(Dictionary::shared().item_named(racket::SIGHT).is_some()),
            "the sight's line where the data has it"
        );
    }

    /// Every version of the free-for-all is a plaque alike (the owner, 2026-11-27: the plain FFA
    /// had none), the plain one first and Pistols last, each with its icon, its name and its share
    /// of the cards, in every language.
    #[test]
    fn every_version_has_its_plaque_the_plain_one_first() {
        for lang in Lang::ALL {
            let page = body(lang, &Pools::every_playable());
            let versions = page.split_once("<h3 class=mode-versions>").unwrap().1;
            let versions = versions.split_once("</h3>").unwrap().1;
            assert!(versions.starts_with("<section class=mode-version id=plain>"), "{}: {versions:.80}", lang.code());
            let plaques: Vec<&str> = versions
                .split("<section class=mode-version id=")
                .skip(1)
                .map(|plaque| plaque.split_once("</section>").unwrap().0)
                .collect();
            let ids: Vec<&str> = plaques.iter().map(|plaque| plaque.split_once('>').unwrap().0).collect();
            assert_eq!(ids, ["plain", "racket", "saloon", "saloon-winchester", "roulette", "pistols"]);
            for plaque in plaques {
                assert!(plaque.contains("><h3><img class=mode-icon src=\"/wiki/icons/modes/"), "{}: {plaque}", lang.code());
                assert!(plaque.contains("<ul class=mode-rules><li>"), "{}: {plaque}", lang.code());
            }
        }
        let page = body(Lang::En, &Pools::every_playable());
        let plain = page.split_once("<section class=mode-version id=plain>").unwrap().1;
        assert!(plain.contains(">Free for all</h3><ul class=mode-rules><li>25 % of FFA cards.</li><li>Everyone plays his own kits.</li></ul>"), "{plain:.300}");
        // The plain one's icon is the mode's own (its maps' `ui.icon`).
        let section = page.split_once("<section class=\"modifiers mode\" id=free-for-all><h2>").unwrap().1;
        let icon = section.split_once("alt=\"\">").unwrap().0;
        assert!(icon.starts_with("<img class=mode-icon src=\"/wiki/icons/modes/"), "{icon}");
        assert!(plain.starts_with(&format!("<h3>{icon}alt=\"\">Free for all</h3>")), "Free for All's own icon: {plain:.200}");
        let pistols = page.split_once("<section class=mode-version id=pistols>").unwrap().1;
        assert!(pistols.contains("src=\"/wiki/icons/modes/twist-18.webp"), "{pistols:.200}");
        assert!(pistols.contains(">FFA: Pistols</h3><ul class=mode-rules><li>10 % of FFA cards.</li><li>Everyone plays the same 2 random kits, each with one pistol; the third kit stays empty.</li><li>Pistols of levels 0–5: TT-33, "), "{pistols:.600}");
        assert!(!page.contains("Kashtan"), "the Kashtan is no pistol");
    }
}

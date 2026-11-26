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
//!   [`roulette`]);
//! - the rooms' offer is the running lobby's pools ([`Pools`]).

use std::collections::BTreeSet;
use std::fmt::Write;

use srp_validate::world::freeze;
use survarium::v026g::cowboy::{self, Variation};
use survarium::v026g::dictionary::{Dictionary, MapId};
use survarium::v026g::item_params;
use survarium::v026g::maps::{self, GameMode, MapMode};
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
    }
}

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

/// A version's icon: the one the loading screen shows for it.
fn version_icon(mode: LobbyMode) -> String {
    icons::url(IconSet::Mode, &format!("twist-{}", mode.number())).map_or_else(String::new, |url| {
        format!("<img class=mode-icon src=\"{}\" alt=\"\">", escape(&url))
    })
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
    item_params::item_params(dict_id)
        .and_then(|params| params.name_key)
        .map_or_else(|| dict_id.to_string(), |key| modifiers::text(lang, key))
}

/// An item's name by its dictionary name, if the data has it.
fn named_item(lang: Lang, name: &str) -> String {
    Dictionary::shared()
        .item_named(name)
        .map_or_else(|| name.to_owned(), |item| item_name(lang, item.dict_id))
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
            vec![
                lang.t("Sniper rifles, silenced guns and guns without automatic fire are replaced."),
                lang.tf(
                    "Rooms up to level {0}: {1}.",
                    &[("0", &up_to), ("1", &gun(Gun::Ak74n))],
                ),
                lang.tf("Other rooms: {0}.", &[("0", &gun(Gun::Ak12))]),
                lang.tf("Sight: {0}.", &[("0", &escape(&named_item(lang, racket::SIGHT)))]),
            ]
        }
        LobbyMode::Cowboy(Variation::Colt) => vec![lang.tf(
            "Every kit: {0} only.",
            &[("0", &escape(&named_item(lang, cowboy::COLT)))],
        )],
        LobbyMode::Cowboy(Variation::WinchesterAndColt) => vec![lang.tf(
            "Every kit: {0} and {1}.",
            &[
                ("0", &escape(&named_item(lang, cowboy::WINCHESTER))),
                ("1", &escape(&named_item(lang, cowboy::COLT))),
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

/// The free-for-all's versions: their subsections.
fn versions(lang: Lang, out: &mut String) {
    let _ = write!(
        out,
        "<h3 class=mode-versions>{}</h3><ul class=mode-rules><li>{}</li></ul>",
        lang.t("Versions"),
        lang.tf("Plain FFA: {0} % of FFA cards.", &[("0", &share(Pick::Plain))]),
    );
    for (mode, pick) in [
        (LobbyMode::Racket, Pick::Racket),
        (LobbyMode::Cowboy(Variation::Colt), Pick::Cowboy(Variation::Colt)),
        (
            LobbyMode::Cowboy(Variation::WinchesterAndColt),
            Pick::Cowboy(Variation::WinchesterAndColt),
        ),
        (LobbyMode::Roulette(Band::Low), Pick::Roulette),
    ] {
        let name = match mode {
            LobbyMode::Roulette(_) => lang.t("FFA: Roulette"),
            other => lobby_mode_name(lang, other),
        };
        let mut lines = vec![lang.tf("{0} % of FFA cards.", &[("0", &share(pick))])];
        lines.extend(version_rules(lang, mode));
        let _ = write!(
            out,
            "<section class=mode-version id={}><h3>{}{}</h3>{}</section>",
            anchor(mode),
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

//! The skills, `/wiki/skills` on the 0.26g site (a section of the wiki,
//! [`crate::wiki`]): every branch of the skill tree as the lobby draws it,
//! with each skill's ranks and values, built by
//! `srp-026g/scripts/skills-tab/build.py` into `../skills/` from the game data
//! of the released patches. Every file is part of the binary; `../skills/files.rs`
//! lists the icons, written by the same build, at their `/skills/` addresses.
//! Above the trees, the levels that give the points ([`levels`]): drawn from the
//! server's own table (`session::v026g::account::level_start`), not the build,
//! with what a match is now (`Results::per_match`) and so how many matches and
//! hours of play a level is. On each skill, how many players took it
//! (`GameAccounts::skill_takers`), counted when the page is asked for.

use std::fmt::Write;
#[cfg(test)]
use std::num::NonZeroU32;

use session::v026g::SkillTakers;
use session::v026g::account::{MAX_LEVEL, level_start};
use session::v026g::rewards::PerMatch;

use crate::Lang;

/// The page's address.
pub(crate) const PATH: &str = "/wiki/skills";

macro_rules! files {
    ($($path:literal => $content_type:literal,)*) => {
        &[$((
            concat!("/skills/", $path),
            $content_type,
            include_bytes!(concat!("../skills/", $path)) as &[u8],
        ),)*]
    };
}

/// The files the page loads: address, content type, bytes.
const FILES: &[(&str, &str, &[u8])] = include!("../skills/files.rs");

/// The page's file at `path` (the target before any `?`): its content type
/// and bytes.
pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    FILES
        .iter()
        .find(|(address, _, _)| *address == path)
        .map(|&(_, content_type, bytes)| (content_type, bytes))
}

/// Where the built body's trees start: the levels go before them.
const TREES: &str = "<div class=\"skill-trees\"";

/// Where a skill of the built body starts, its id next; its disc follows.
const SKILL: &str = "<div class=\"skill\" tabindex=\"0\" data-skill=\"";
const DISC: &str = "<span class=\"skill-disc\">";

/// What the page shows of the running server.
#[derive(Default)]
pub(crate) struct Now {
    /// What a match gives now (`None`: no match to go by yet, and no number
    /// of matches).
    pub(crate) per_match: Option<PerMatch>,
    /// How many players took each skill.
    pub(crate) takers: SkillTakers,
}

/// The page's body in `lang`, for the wiki's page, with what it shows of
/// the server `now`.
pub(crate) fn body(lang: Lang, now: &Now) -> String {
    let built = match lang {
        Lang::En => include_str!("../skills/skills.en.html"),
        Lang::Uk => include_str!("../skills/skills.uk.html"),
    };
    let at = built
        .find(TREES)
        .expect("the built skills page has its trees");
    format!(
        "{}{}{}",
        &built[..at],
        levels(lang, now.per_match),
        with_takers(lang, &built[at..], &now.takers)
    )
}

/// `trees` with, on each skill's disc, how many players took it.
fn with_takers(lang: Lang, trees: &str, takers: &SkillTakers) -> String {
    let mut out = String::with_capacity(trees.len() + 4096);
    let mut rest = trees;
    while let Some(at) = rest.find(SKILL) {
        let (head, tail) = rest.split_at(at + SKILL.len());
        let id = tail
            .split_once('"')
            .and_then(|(id, _)| id.parse().ok())
            .expect("a built skill's id");
        let disc = tail.find(DISC).expect("a built skill's disc") + DISC.len();
        out.push_str(head);
        out.push_str(&tail[..disc]);
        let _ = write!(
            out,
            "<span class=skill-takers>{}</span>",
            thousands(lang, takers.of(id))
        );
        rest = &tail[disc..];
    }
    out.push_str(rest);
    out
}

/// `n` with its thousands apart, as `lang` writes them.
fn thousands(lang: Lang, n: u32) -> String {
    let digits = n.to_string();
    let gap = match lang {
        Lang::En => ",",
        Lang::Uk => "\u{a0}",
    };
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(gap);
        }
        out.push(digit);
    }
    out
}

/// The hours of play `experience` takes at what a match is now: tenths
/// under ten hours, whole hours from there.
fn hours(lang: Lang, experience: u32, per_match: PerMatch) -> String {
    let tenths = (u64::from(experience) * u64::from(per_match.seconds.get()) * 10)
        .div_ceil(u64::from(per_match.experience.get()) * 3600);
    if tenths < 100 {
        let point = match lang {
            Lang::En => '.',
            Lang::Uk => ',',
        };
        format!("{}{point}{}", tenths / 10, tenths % 10)
    } else {
        thousands(lang, u32::try_from(tenths.div_ceil(10)).unwrap_or(u32::MAX))
    }
}

/// The experience level `level` (1 or more) takes on its own: from the one below to it.
fn cost(level: u8) -> u32 {
    level_start(level) - level_start(level - 1)
}

/// The levels the table lists.
const LISTED: [u8; 12] = [1, 5, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];

/// The graph's box and plot area, in its own units.
const WIDTH: f64 = 640.0;
const HEIGHT: f64 = 300.0;
const LEFT: f64 = 58.0;
const RIGHT: f64 = 620.0;
const TOP: f64 = 14.0;
const BOTTOM: f64 = 262.0;
/// The graph's experience lines, apart.
const STEP: u32 = 250_000;

/// The levels section: what a level gives, how much experience it takes
/// as two formulas (a level from the one before, the total), a
/// table of some levels (with the matches and hours each is at what a match
/// is now) and a graph of the total.
fn levels(lang: Lang, per_match: Option<PerMatch>) -> String {
    let (first, step) = (cost(1), cost(2) - cost(1));
    // The formulas' names: a level's own experience, and the total to it.
    let (exp, total) = match lang {
        Lang::En => ("Exp", "Total"),
        Lang::Uk => ("Досвід", "Усього"),
    };
    let lines = level_start(MAX_LEVEL).div_ceil(STEP);
    let y_max = f64::from(lines * STEP);
    let x = |level: u8| LEFT + (RIGHT - LEFT) * f64::from(level) / f64::from(MAX_LEVEL);
    let y = |total: u32| BOTTOM - (BOTTOM - TOP) * f64::from(total) / y_max;
    let short = |total: u32| match total {
        0 => "0".to_owned(),
        t if t >= 1_000_000 => format!("{}M", f64::from(t) / 1e6),
        t => format!("{}k", t / 1000),
    };
    let rule = match lang {
        Lang::En => "Experience gives levels, and every level gives one skill point.",
        Lang::Uk => "Досвід дає рівні, а кожен рівень — одне очко навичок.",
    };
    let mut out = String::new();
    let _ = write!(
        out,
        "<section class=levels><h2>{}</h2><div class=levels-body><div class=levels-text>\
         <p>{rule}</p>\
         <p class=levels-formula>{exp}<sub>L</sub> <span>= {exp}<sub>L−1</sub> + {}, \
         {exp}<sub>1</sub> = {}</span></p>\
         <p class=levels-formula>{total}<sub>L</sub> <span>= {} × L + {} × L × (L − 1)</span></p>\
         <div class=table-scroll><table class=protection><thead><tr><th>{}</th><th>{}</th>\
         <th>{}</th>{}</tr></thead><tbody>",
        lang.tr("Levels", "Рівні"),
        thousands(lang, step),
        thousands(lang, first),
        thousands(lang, first),
        thousands(lang, step / 2),
        lang.tr("Level", "Рівень"),
        lang.tr("This level", "Цей рівень"),
        lang.tr("Total", "Усього"),
        per_match.map_or(String::new(), |_| format!(
            "<th>{}</th><th>{}</th>",
            lang.tr("Matches", "Матчів"),
            lang.tr("Hours", "Годин")
        )),
    );
    for level in LISTED {
        // How many matches and hours of play the level is at what a match is now.
        let matches = per_match.map_or(String::new(), |per_match| {
            let total = level_start(level);
            let matches = total.div_ceil(per_match.experience.get());
            format!(
                "<td>≈ {}</td><td>≈ {}</td>",
                thousands(lang, matches),
                hours(lang, total, per_match)
            )
        });
        let _ = write!(
            out,
            "<tr><th>{level}</th><td>{}</td><td>{}</td>{matches}</tr>",
            thousands(lang, cost(level)),
            thousands(lang, level_start(level))
        );
    }
    out.push_str("</tbody></table></div></div>");
    // The graph: the total by level; hovering a level shows its numbers.
    let _ = write!(
        out,
        "<figure class=levels-graph><svg viewBox=\"0 0 {WIDTH} {HEIGHT}\" role=img aria-label=\"{}\">",
        lang.tr("Total experience by level", "Досвід усього за рівнем"),
    );
    for line in 0..=lines {
        let total = line * STEP;
        let _ = write!(
            out,
            "<line class=grid x1={LEFT} x2={RIGHT} y1={0:.1} y2={0:.1} />\
             <text class=axis x={1} y={0:.1} dy=4 text-anchor=end>{2}</text>",
            y(total),
            LEFT - 8.0,
            short(total)
        );
    }
    for level in (0..=MAX_LEVEL).step_by(10) {
        let _ = write!(
            out,
            "<text class=axis x={:.1} y={} text-anchor=middle>{level}</text>",
            x(level),
            BOTTOM + 18.0
        );
    }
    let _ = write!(
        out,
        "<text class=axis-name x={} y={} text-anchor=middle>{}</text>",
        (LEFT + RIGHT) / 2.0,
        HEIGHT - 2.0,
        lang.tr("Level", "Рівень")
    );
    let points: Vec<String> = (0..=MAX_LEVEL)
        .map(|level| format!("{:.1},{:.1}", x(level), y(level_start(level))))
        .collect();
    let _ = write!(
        out,
        "<polyline class=curve points=\"{}\" />",
        points.join(" ")
    );
    let band = (RIGHT - LEFT) / f64::from(MAX_LEVEL);
    for level in 1..=MAX_LEVEL {
        let (px, py) = (x(level), y(level_start(level)));
        let (anchor, dx) = if px > (LEFT + RIGHT) / 2.0 {
            ("end", -10.0)
        } else {
            ("start", 10.0)
        };
        let _ = write!(
            out,
            "<g class=lv><rect x={:.1} y={TOP} width={band:.2} height={} />\
             <line x1={px:.1} x2={px:.1} y1={py:.1} y2={BOTTOM} />\
             <circle cx={px:.1} cy={py:.1} r=4 />\
             <text x={:.1} y={:.1} text-anchor={anchor}>{} {level} · {}</text></g>",
            px - band / 2.0,
            BOTTOM - TOP,
            px + dx,
            (py - 10.0).max(TOP + 12.0),
            lang.tr("Level", "Рівень"),
            thousands(lang, level_start(level)),
        );
    }
    out.push_str("</svg></figure></div></section>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skill_shows_how_many_players_took_it() {
        let accounts = session::v026g::GameAccounts::new();
        for id in 1..=1234 {
            accounts
                .get_or_create(id, &format!("p{id}"))
                .lock()
                .unwrap()
                .skills = vec![(101, 1)];
        }
        let now = Now {
            takers: accounts.skill_takers(),
            ..Now::default()
        };
        for lang in [Lang::En, Lang::Uk] {
            let page = body(lang, &now);
            let badges = page.matches("<span class=skill-takers>").count();
            assert_eq!(badges, page.matches(SKILL).count(), "{lang:?}");
            assert!(badges > 40, "{lang:?}");
        }
        let page = body(Lang::En, &now);
        let quick = &page[page.find(&format!("{SKILL}101\"")).unwrap()..];
        assert!(quick.contains(&format!("{DISC}<span class=skill-takers>1,234</span>")));
        let other = &page[page.find(&format!("{SKILL}102\"")).unwrap()..];
        assert!(other.contains(&format!("{DISC}<span class=skill-takers>0</span>")));
    }

    #[test]
    fn the_formula_is_the_servers_table() {
        let (first, step) = (cost(1), cost(2) - cost(1));
        assert!(step.is_multiple_of(2), "the formula halves the step");
        for level in 2..=MAX_LEVEL {
            assert_eq!(cost(level), cost(level - 1) + step, "level {level}");
        }
        for level in 0..=MAX_LEVEL {
            let l = u32::from(level);
            let formula = first * l + step / 2 * l * l.saturating_sub(1);
            assert_eq!(formula, level_start(level), "level {level}");
        }
    }

    #[test]
    fn levels_go_before_the_trees_in_both_languages() {
        for lang in [Lang::En, Lang::Uk] {
            let page = body(lang, &Now::default());
            let levels = page.find("<section class=levels>").expect("the levels");
            assert!(levels < page.find(TREES).expect("the trees"));
            assert!(page.contains(&thousands(lang, level_start(MAX_LEVEL))));
            assert!(!page.contains("≈"), "no matches without a match to go by");
        }
        // At 1,000 in 12 minutes a match: level 100 is 1,288 matches and
        // 258 hours (257.5), level 1 one match and 0.1 h.
        let per_match = PerMatch {
            experience: NonZeroU32::new(1000).unwrap(),
            seconds: NonZeroU32::new(720).unwrap(),
        };
        let page = body(
            Lang::En,
            &Now {
                per_match: Some(per_match),
                ..Now::default()
            },
        );
        assert!(page.contains("<td>1,287,500</td><td>≈ 1,288</td><td>≈ 258</td>"));
        assert!(page.contains("<th>1</th><td>500</td><td>500</td><td>≈ 1</td><td>≈ 0.1</td>"));
        assert_eq!(hours(Lang::Uk, 25_000, per_match), "5,0");
        assert_eq!(thousands(Lang::En, 1_287_500), "1,287,500");
        assert_eq!(thousands(Lang::Uk, 830_000), "830\u{a0}000");
        assert_eq!(thousands(Lang::En, 500), "500");
    }
}

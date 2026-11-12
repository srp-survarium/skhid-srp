//! The one page: notices, then the items grouped by state. Every value from the data file goes
//! through `esc`; the page has no script and loads nothing from elsewhere.

use crate::model::{Board, Date, Item, Source, State};
use std::fmt::Write;

/// Done items stay on the page this many days after their last update.
pub const DONE_DAYS: i64 = 14;

const PR_URL: &str = "https://github.com/srp-survarium/skhid-srp/pull/";

/// HTML-escape text for element content and quoted attribute values.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

struct Section {
    id: &'static str,
    title: &'static str,
    state: State,
}

const SECTIONS: [Section; 7] = [
    Section { id: "waiting", title: "Waiting on owner", state: State::Waiting },
    Section { id: "in-progress", title: "In progress", state: State::InProgress },
    Section { id: "review", title: "In review", state: State::Review },
    Section { id: "next", title: "Next", state: State::Next },
    Section { id: "requests", title: "Requests", state: State::Requested },
    Section { id: "done", title: "Recently done", state: State::Done },
    Section { id: "parked", title: "Parked", state: State::Parked },
];

/// Render the page. `banner` is shown above everything (the data file was rejected).
pub fn page(board: Option<&Board>, banner: Option<&str>, today: Date) -> String {
    let mut h = String::with_capacity(16 * 1024);
    h.push_str(HEAD);
    h.push_str("<main>\n<header><h1>Skhid status</h1>");
    if let Some(b) = board {
        let _ = write!(h, "<p class=\"updated\">Updated {}</p>", esc(&b.updated));
    }
    h.push_str("</header>\n");

    if let Some(msg) = banner {
        let _ = writeln!(h, "<div class=\"banner\" role=\"alert\">{}</div>", esc(msg));
    }

    let Some(b) = board else {
        h.push_str("<p class=\"empty\">Nothing to show yet.</p>\n</main>\n</body>\n</html>\n");
        return h;
    };

    // Items per section, newest first; done ones only while recent.
    let mut groups: Vec<Vec<&Item>> = SECTIONS
        .iter()
        .map(|s| b.items.iter().filter(|i| i.state == s.state).collect())
        .collect();
    let mut old_done = 0;
    for (s, g) in SECTIONS.iter().zip(groups.iter_mut()) {
        if s.state == State::Done {
            let before = g.len();
            g.retain(|i| today.days() - i.updated.days() <= DONE_DAYS);
            old_done = before - g.len();
        }
        g.sort_by(|a, b| {
            let src = |i: &Item| if s.state == State::Requested { i.source } else { Source::Dev };
            src(a).cmp(&src(b)).then(b.updated.cmp(&a.updated)).then(a.n.cmp(&b.n))
        });
    }

    // Counts bar: one link per non-empty section.
    let counts: Vec<String> = SECTIONS
        .iter()
        .zip(&groups)
        .filter(|(_, g)| !g.is_empty())
        .map(|(s, g)| {
            format!(
                "<a href=\"#{}\" class=\"count count-{}\"><b>{}</b> {}</a>",
                s.id,
                s.id,
                g.len(),
                s.title.to_lowercase()
            )
        })
        .collect();
    if !counts.is_empty() {
        let _ = writeln!(h, "<nav class=\"counts\">{}</nav>", counts.join(""));
    }

    if !b.notices.is_empty() {
        let mut notices: Vec<_> = b.notices.iter().collect();
        notices.sort_by_key(|n| std::cmp::Reverse(n.date));
        h.push_str("<section class=\"notices\" aria-label=\"Notices\"><ul>\n");
        for n in notices {
            let _ = writeln!(
                h,
                "<li class=\"notice {}\"><time datetime=\"{}\">{}</time><span>{}</span></li>",
                n.level.name(),
                n.date,
                n.date.short(),
                esc(&n.text)
            );
        }
        h.push_str("</ul></section>\n");
    }

    for (s, g) in SECTIONS.iter().zip(&groups) {
        if g.is_empty() {
            continue;
        }
        if s.state == State::Parked {
            let _ = writeln!(
                h,
                "<details class=\"section\" id=\"{}\"><summary><h2>{} <span class=\"n\">{}</span></h2></summary>",
                s.id,
                s.title,
                g.len()
            );
        } else {
            let _ = writeln!(
                h,
                "<section class=\"section s-{}\" id=\"{}\"><h2>{} <span class=\"n\">{}</span></h2>",
                s.id,
                s.id,
                s.title,
                g.len()
            );
        }
        if s.state == State::Requested {
            for (src, label) in
                [(Source::Owner, "From the owner"), (Source::Player, "From players"), (Source::Dev, "From the team")]
            {
                let part: Vec<&Item> = g.iter().copied().filter(|i| i.source == src).collect();
                if !part.is_empty() {
                    let _ = writeln!(h, "<h3>{label}</h3>");
                    list(&mut h, &part);
                }
            }
        } else {
            list(&mut h, g);
        }
        if s.state == State::Done && old_done > 0 {
            let _ = writeln!(h, "<p class=\"more\">{old_done} older done item(s) not shown.</p>");
        }
        h.push_str(if s.state == State::Parked { "</details>\n" } else { "</section>\n" });
    }
    if groups.iter().all(Vec::is_empty) && old_done > 0 {
        let _ = writeln!(h, "<p class=\"more\">{old_done} older done item(s) not shown.</p>");
    }

    h.push_str("<footer>Read-only page. Ask for changes; they arrive through the data file.</footer>\n");
    h.push_str("</main>\n</body>\n</html>\n");
    h
}

fn list(h: &mut String, items: &[&Item]) {
    h.push_str("<ul class=\"items\">\n");
    for i in items {
        item(h, i);
    }
    h.push_str("</ul>\n");
}

fn item(h: &mut String, i: &Item) {
    let _ = write!(h, "<li class=\"item\" id=\"t{n}\"><a class=\"num\" href=\"#t{n}\">T{n}</a><div class=\"body\">", n = i.n);
    let _ = write!(
        h,
        "<div class=\"line\"><span class=\"title\">{}</span>",
        esc(&i.title),
    );
    let _ = write!(h, "<span class=\"chip src-{0}\">{0}</span>", i.source.name());
    if let Some(a) = &i.area {
        let _ = write!(h, "<span class=\"chip\">{}</span>", esc(a));
    }
    h.push_str("<span class=\"side\">");
    for pr in &i.prs {
        let _ = write!(h, "<a class=\"pr\" href=\"{PR_URL}{pr}\" rel=\"noreferrer\">#{pr}</a>");
    }
    let _ = write!(h, "<time datetime=\"{}\">{}</time></span></div>", i.updated, i.updated.short());
    let _ = write!(h, "<div class=\"next\">{}</div>", esc(&i.next));
    if !i.requested_by.is_empty() {
        let names: Vec<String> = i.requested_by.iter().map(|n| esc(n)).collect();
        let _ = write!(h, "<div class=\"by\">Asked by {}</div>", names.join(", "));
    }
    if i.detail.is_some() || !i.links.is_empty() {
        h.push_str("<details class=\"more-info\"><summary>Details</summary>");
        if let Some(d) = &i.detail {
            let _ = write!(h, "<div class=\"detail\">{}</div>", esc(d));
        }
        if !i.links.is_empty() {
            h.push_str("<ul class=\"links\">");
            for l in &i.links {
                let _ = write!(
                    h,
                    "<li><a href=\"{}\" rel=\"noreferrer\">{}</a></li>",
                    esc(&l.url),
                    esc(&l.label)
                );
            }
            h.push_str("</ul>");
        }
        h.push_str("</details>");
    }
    h.push_str("</div></li>\n");
}

/// The 404 page: says nothing about what exists.
pub const NOT_FOUND: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Not found</title></head><body><p>Not found.</p></body></html>\n";

const HEAD: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="robots" content="noindex,nofollow">
<meta name="referrer" content="no-referrer">
<meta name="color-scheme" content="light dark">
<title>Skhid status</title>
<link rel="icon" href="data:,">
<style>
:root{--bg:#f5f6f8;--panel:#fff;--text:#1a1d23;--muted:#5d6876;--line:#e2e5ea;--chip:#eef0f3;
--accent:#2557d6;--warn:#b25e09;--ok:#16803c;--bad:#c0262d;--owner:#7442c8;--player:#0b7285;
--wait-bg:#fff7ed}
@media (prefers-color-scheme:dark){:root{--bg:#0e1014;--panel:#161920;--text:#e4e7eb;--muted:#9099a6;
--line:#252a33;--chip:#21262f;--accent:#7ea2ff;--warn:#f2a444;--ok:#4cc77a;--bad:#ff6b6b;--owner:#b597f5;
--player:#5cc4d6;--wait-bg:#221a10}}
*{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{margin:0;background:var(--bg);color:var(--text);
font:14px/1.45 system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif}
main{max-width:920px;margin:0 auto;padding:20px 16px 40px}
a{color:var(--accent);text-decoration:none}a:hover{text-decoration:underline}
header{display:flex;flex-wrap:wrap;align-items:baseline;justify-content:space-between;gap:4px 16px;margin-bottom:12px}
h1{font-size:20px;margin:0;letter-spacing:-.01em}
.updated{margin:0;color:var(--muted);font-size:13px}
.banner{border:1px solid var(--bad);color:var(--bad);background:var(--panel);border-radius:8px;
padding:8px 12px;margin:0 0 12px;white-space:pre-line;font-size:13px}
.counts{display:flex;flex-wrap:wrap;gap:6px;margin:0 0 14px}
.count{background:var(--panel);border:1px solid var(--line);border-radius:999px;padding:2px 10px;
color:var(--muted);font-size:12.5px}
.count b{color:var(--text);font-weight:600}
.count-waiting{border-color:var(--warn)}
.notices ul{list-style:none;margin:0 0 18px;padding:0;display:grid;gap:6px}
.notice{display:flex;gap:10px;align-items:baseline;background:var(--panel);border:1px solid var(--line);
border-left:3px solid var(--accent);border-radius:6px;padding:7px 12px}
.notice.warn{border-left-color:var(--warn)}.notice.done{border-left-color:var(--ok)}
.notice time{color:var(--muted);font-size:12px;white-space:nowrap;min-width:44px;font-variant-numeric:tabular-nums}
.section{margin:0 0 18px}
h2{font-size:13px;text-transform:uppercase;letter-spacing:.06em;color:var(--muted);margin:0 0 6px;font-weight:600}
h2 .n{font-weight:500;opacity:.8}
.s-waiting h2{color:var(--warn)}
h3{font-size:12px;color:var(--muted);font-weight:600;margin:10px 0 4px}
details.section>summary{cursor:pointer;list-style:none}
details.section>summary::-webkit-details-marker{display:none}
details.section>summary h2{display:inline}
details.section>summary h2::before{content:"+ ";font-weight:400}
details.section[open]>summary h2::before{content:"- "}
details.section[open]>summary{margin-bottom:6px}
.items{list-style:none;margin:0;padding:0;background:var(--panel);border:1px solid var(--line);border-radius:8px;overflow:hidden}
.s-waiting .items{background:var(--wait-bg)}
.item:first-child{border-top:0}
.item{display:grid;grid-template-columns:30px minmax(0,1fr);column-gap:8px;padding:8px 12px;border-top:1px solid var(--line)}
.item:target{outline:2px solid var(--accent);outline-offset:-2px}
.line{display:flex;flex-wrap:wrap;align-items:baseline;gap:4px 8px}
.num{font:600 12px/20px ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:var(--muted)}
.title{font-weight:600}
.chip{font-size:11.5px;line-height:18px;padding:0 7px;border-radius:4px;background:var(--chip);color:var(--muted)}
.src-owner{color:var(--owner)}.src-player{color:var(--player)}
.side{margin-left:auto;display:flex;gap:8px;align-items:baseline;font-size:12.5px}
.side time{color:var(--muted);white-space:nowrap;font-variant-numeric:tabular-nums}
.pr{font-variant-numeric:tabular-nums}
.next,.by{margin:2px 0 0;color:var(--muted)}
.by{font-size:12.5px}
.more-info{margin:4px 0 0;font-size:13px}
.more-info summary{cursor:pointer;color:var(--muted);font-size:12.5px;width:max-content}
.detail{white-space:pre-line;margin:4px 0}
.links{margin:4px 0;padding-left:18px}
.more,.empty{color:var(--muted);font-size:12.5px;margin:6px 2px 0}
footer{margin-top:28px;color:var(--muted);font-size:12px}
@media (max-width:560px){main{padding:14px 12px 32px}.item{padding:8px 10px;column-gap:6px}
.side{margin-left:0}}
</style>
</head>
<body>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse;

    const TODAY: Date = Date { y: 2026, m: 10, d: 23 };

    #[test]
    fn escapes_everything_from_the_file() {
        assert_eq!(esc(r#"<a href="x">'&'</a>"#), "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;");
        let b = parse(
            r#"{"updated":"<b>now</b>","notices":[{"date":"2026-10-23","text":"<script>alert(1)</script>"}],
            "items":[{"n":7,"id":"x","title":"<img src=x onerror=alert(1)>","state":"next","next":"a&b",
            "detail":"\"quoted\"","requested_by":["<i>me</i>"],
            "links":[{"label":"<l>","url":"https://e.x/?a=1&b=\"2\""}],"updated":"2026-10-23"}]}"#,
        )
        .unwrap();
        let p = page(Some(&b), Some("<bad>"), TODAY);
        assert!(!p.contains("<script>alert"));
        assert!(!p.contains("<img src=x"));
        assert!(!p.contains("<b>now"));
        assert!(!p.contains("<i>me"));
        assert!(!p.contains("<bad>"));
        assert!(p.contains("&lt;img src=x onerror=alert(1)&gt;"));
        assert!(p.contains("href=\"https://e.x/?a=1&amp;b=&quot;2&quot;\""));
        assert!(!p.contains("<script"));
    }

    #[test]
    fn sections_in_order_and_old_done_hidden() {
        let b = parse(include_str!("../example.json")).unwrap();
        let p = page(Some(&b), None, TODAY);
        let at = |s: &str| p.find(s).unwrap_or_else(|| panic!("{s} missing"));
        let order = ["id=\"waiting\"", "id=\"in-progress\"", "id=\"review\"", "id=\"next\"", "id=\"requests\"",
            "id=\"done\"", "id=\"parked\""];
        for w in order.windows(2) {
            assert!(at(w[0]) < at(w[1]), "{} before {}", w[0], w[1]);
        }
        assert!(at("From the owner") < at("From players"));
        assert!(p.contains("href=\"https://github.com/srp-survarium/skhid-srp/pull/"));
        assert!(p.contains("older done item(s) not shown"));
        assert!(!p.contains("Ancient fix"), "done items older than 14 days are hidden");
    }

    #[test]
    fn banner_without_board() {
        let p = page(None, Some("board.json was rejected"), TODAY);
        assert!(p.contains("class=\"banner\""));
        assert!(p.contains("Nothing to show yet"));
    }
}

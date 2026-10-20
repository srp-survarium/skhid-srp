"""The wiki's HTML: the page shell in the site's look (the calculator's and
the Maps tab's: paper and ink, PT Sans, PT Sans Narrow headings, PT Mono
numbers, light or dark as the system is), and the pieces pages are made of.
Every page is written once per language; `L` carries the language."""
import html

SITE = "/wiki"


def esc(text):
    return html.escape(str(text), quote=True)


class L:
    """The language a page is written in."""

    def __init__(self, code):
        self.code = code

    def t(self, en, uk):
        return uk if self.code == "uk" else en

    def n(self, text):
        """A {en, uk} text in this language."""
        if text is None:
            return ""
        if isinstance(text, str):
            return text
        if isinstance(text, tuple):
            return self.t(*text)
        return text.get(self.code) or text.get("en") or ""

    def href(self, slug=""):
        return f"{SITE}/{slug}?lang={self.code}" if slug else f"{SITE}?lang={self.code}"

    def a(self, slug, text):
        return f'<a href="{self.href(slug)}">{esc(self.n(text))}</a>'

    def num(self, value, digits=2):
        """A number as the page prints it: no trailing zeros, thousands
        grouped (a thin space in Ukrainian, a comma in English)."""
        if value is None:
            return "—"
        value = float(value)
        text = f"{value:,.{digits}f}"
        if "." in text:
            text = text.rstrip("0").rstrip(".")
        if text in ("-0", ""):
            text = "0"
        if self.code == "uk":
            text = text.replace(",", " ").replace(".", ",")
        return text

    def hp(self, value):
        """Health, damage, armour and penetration as the game's cards show
        them: x100."""
        return self.num(float(value) * 100, 1)

    def pct(self, value, digits=1, sign=False):
        text = self.num(float(value) * 100, digits)
        if sign and float(value) > 0:
            text = "+" + text
        return text + " %"

    def sec(self, value):
        return self.num(value) + self.t(" s", " с")

    def metres(self, value):
        return self.num(value, 1) + self.t(" m", " м")

    def kg(self, value):
        return self.num(value) + self.t(" kg", " кг")

    def money(self, cost, gold=False):
        return self.num(cost, 0) + " " + (self.t("gold", "золота") if gold else self.t("silver", "срібла"))


def table(L, head, body, sort=True, cls=""):
    """A table: head is a list of column titles (a title starting with '#'
    is a number column), body a list of rows of HTML cells; `sort` lets the
    reader sort by any column (wiki.js)."""
    out = [f'<div class="scroll"><table class="{"sort " if sort else ""}{cls}"><thead><tr>']
    for h in head:
        numeric = h.startswith("#")
        out.append(f'<th{" class=n" if numeric else ""}>{h.lstrip("#")}</th>')
    out.append("</tr></thead><tbody>")
    for row in body:
        out.append("<tr>")
        for i, cell in enumerate(row):
            numeric = i < len(head) and head[i].startswith("#")
            if isinstance(cell, tuple):  # (shown, sort key)
                shown, key = cell
                out.append(f'<td{" class=n" if numeric else ""} data-v="{esc(key)}">{shown}</td>')
            else:
                out.append(f'<td{" class=n" if numeric else ""}>{cell}</td>')
        out.append("</tr>")
    out.append("</tbody></table></div>")
    return "".join(out)


def infobox(L, title, icon, rows_, sections=()):
    """The item card at the top of an item page: the lobby icon and rows of
    label and value; `sections` are (heading, rows) after them."""
    out = ['<aside class="box">']
    if icon:
        out.append(f'<img class="pic" src="{icon}" alt="" width="171" height="83">')
    out.append(f"<h2>{esc(title)}</h2><dl>")
    for label, value in rows_:
        if value not in (None, ""):
            out.append(f"<dt>{label}</dt><dd>{value}</dd>")
    out.append("</dl>")
    for heading, more in sections:
        more = [(k, v) for k, v in more if v not in (None, "")]
        if more:
            out.append(f"<h3>{heading}</h3><dl>")
            out.extend(f"<dt>{k}</dt><dd>{v}</dd>" for k, v in more)
            out.append("</dl>")
    out.append("</aside>")
    return "".join(out)


STYLE = """
:root {
  --bg: #e9e6dc; --panel: #f5f3ec; --ink: #1f2420; --ink-2: #4d544c; --ink-3: #7b8277; --rule: #c9c5b6;
  --accent: #b4501f; --good: #2f7a3a; --bad: #a3322a;
  --font-display: "PT Sans Narrow", "Arial Narrow", "Roboto Condensed", sans-serif;
  --font-body: "PT Sans", "Segoe UI", Roboto, sans-serif;
  --font-data: "PT Mono", ui-monospace, "SFMono-Regular", Consolas, monospace;
}
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) {
  --bg: #171a17; --panel: #1f231f; --ink: #e6e4da; --ink-2: #b4b6a8; --ink-3: #868a7e; --rule: #353a33;
  --accent: #e07a43; --good: #6fbf73; --bad: #e0675c; color-scheme: dark } }
:root[data-theme="dark"] {
  --bg: #171a17; --panel: #1f231f; --ink: #e6e4da; --ink-2: #b4b6a8; --ink-3: #868a7e; --rule: #353a33;
  --accent: #e07a43; --good: #6fbf73; --bad: #e0675c; color-scheme: dark }
* { box-sizing: border-box }
body { background: var(--bg); color: var(--ink); font: 15px/1.55 var(--font-body); margin: 0;
  padding-left: max(16px, env(safe-area-inset-left, 0px)); padding-right: max(16px, env(safe-area-inset-right, 0px)); padding-block: 20px 48px }
.wrap { max-width: 1100px; margin: 0 auto; display: flow-root }
a { color: var(--accent) }
.crumbs { font: .8rem var(--font-data); color: var(--ink-3); margin: 0 0 6px }
.crumbs a { color: var(--ink-2); text-decoration: none } .crumbs a:hover { color: var(--accent) }
h1 { font: 700 2.1rem/1.05 var(--font-display); margin: 0 0 14px; border-bottom: 2px solid var(--ink); padding-bottom: 10px }
h2 { font: 700 1.3rem var(--font-display); text-transform: uppercase; letter-spacing: .06em; margin: 26px 0 8px; border-bottom: 1px solid var(--rule); padding-bottom: 4px }
h3 { font: 700 1rem var(--font-display); text-transform: uppercase; letter-spacing: .08em; color: var(--ink-2); margin: 18px 0 6px }
p, ul, ol { margin: 0 0 10px; max-width: 46rem } li { margin: .2rem 0 }
.lead { color: var(--ink-2) }
.muted, small { color: var(--ink-3) }
.n, td.n, th.n { font-family: var(--font-data); text-align: right; white-space: nowrap }
code { font: .85rem var(--font-data); background: var(--panel); border: 1px solid var(--rule); padding: 0 .25rem; border-radius: 2px }
pre.formula { margin: 6px 0 12px; padding: 10px 12px; font: .85rem/1.65 var(--font-data); background: var(--panel); border: 1px solid var(--rule); overflow-x: auto; white-space: pre }
.box { float: right; width: 300px; margin: 0 0 16px 22px; background: var(--panel); border: 1px solid var(--rule); padding: 12px 14px }
.box .pic { display: block; width: 100%; height: auto; margin: 0 auto 6px; image-rendering: auto }
.box h2 { margin: 0 0 6px; font-size: 1.1rem; border: 0; padding: 0; text-transform: none; letter-spacing: .02em }
.box h3 { margin: 12px 0 4px; font-size: .8rem; border-top: 1px solid var(--rule); padding-top: 8px }
.box dl { display: grid; grid-template-columns: auto 1fr; gap: 3px 12px; margin: 0; font-size: .88rem }
.box dt { color: var(--ink-3) } .box dd { margin: 0; text-align: right; font-family: var(--font-data) }
@media (max-width: 760px) { .box { float: none; width: auto; margin: 0 0 16px } }
.scroll { overflow-x: auto; margin: 6px 0 14px; max-width: 100% }
table { border-collapse: collapse; font-size: .88rem; min-width: 50% }
th, td { text-align: left; padding: .3rem .5rem; border-bottom: 1px solid var(--rule); vertical-align: middle }
th { font: 700 .72rem var(--font-display); letter-spacing: .1em; text-transform: uppercase; color: var(--ink-3); white-space: nowrap }
table.sort th { cursor: pointer; user-select: none } table.sort th:hover { color: var(--ink) }
table.sort th[aria-sort=ascending]::after { content: " ▲" } table.sort th[aria-sort=descending]::after { content: " ▼" }
tbody tr:hover { background: var(--panel) }
td img.ic { width: 86px; height: 42px; object-fit: contain; display: block }
td.pos { color: var(--good) } td.neg { color: var(--bad) }
.tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 10px; margin: 10px 0 18px }
.tile { display: grid; gap: 4px; align-content: start; background: var(--panel); border: 1px solid var(--rule); padding: 10px 12px; text-decoration: none; color: var(--ink) }
.tile:hover { border-color: var(--accent) }
.tile b { font: 700 1.05rem var(--font-display); text-transform: uppercase; letter-spacing: .06em; color: var(--accent) }
.tile span { font-size: .82rem; color: var(--ink-2) }
.tile img { width: 100%; height: 64px; object-fit: contain }
.find { width: min(100%, 22rem); padding: .4rem .6rem; background: var(--panel); color: inherit; border: 1px solid var(--rule); border-radius: 2px; font: inherit; margin: 4px 0 8px }
.find:focus { outline: 0; border-color: var(--accent) }
.cols { columns: 16rem; column-gap: 24px } .cols li { break-inside: avoid }
.tag { font: 700 .68rem var(--font-display); letter-spacing: .08em; text-transform: uppercase; border: 1px solid var(--rule); padding: 0 .35rem; border-radius: 2px; color: var(--ink-2) }
.stamp { font: .75rem var(--font-data); color: var(--ink-3); margin-top: 28px; border-top: 1px solid var(--rule); padding-top: 8px; clear: both }
"""


def page(L, title, body, crumbs, faces, script, stamp):
    """A whole page: {{LANG}} is filled already (one file a language);
    <!--SITE-HEADER--> is the server's."""
    trail = "".join(f"{L.a(s, t)} / " for s, t in crumbs)
    return ("<!doctype html>\n"
            f'<html lang="{L.code}">\n<meta charset="utf-8">\n'
            '<meta name="viewport" content="width=device-width,initial-scale=1">\n'
            f"<title>{esc(title)} · {L.t('Skhid wiki', 'Вікі Сходу')}</title>\n"
            f"<style>\n{faces}\n{STYLE}</style>\n<!--SITE-HEADER-->\n"
            f'<div class="wrap">\n<p class="crumbs">{trail}</p>\n<h1>{esc(title)}</h1>\n{body}\n'
            f'<p class="stamp">{esc(stamp)}</p>\n</div>\n'
            f'<script src="{script}"></script>\n')

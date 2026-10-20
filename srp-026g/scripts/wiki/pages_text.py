"""The hand-written pages: text/<slug>.<en|uk>.md, short and plain. Their
numbers are never typed: `{{name}}` is a fact of the game data or of srp
(`facts`), `{{table:name}}` a generated table; an unknown name stops the
build. The format, line by line:

    # Title                      the page's title (first line)
    section: mechanics           where the main page lists it
    summary: one line            for the main page and the glossary
    ## Heading
    - a bullet
    | a | b |                    a table (the first row is its head)
    ```                          a formula block, until the next ```
    plain text                   a paragraph (lines join until a blank one)

and inline [text](slug) (a wiki page, or /path for the site), **bold**,
`code`.
"""
import re
from pathlib import Path

from web import esc

TEXT = Path(__file__).resolve().parent / "text"


def inline(L, text, facts, tables):
    def fact(m):
        name = m.group(1)
        if name not in facts:
            raise SystemExit(f"text: no fact {{{{{name}}}}}")
        return esc(facts[name](L))

    out = esc(text)
    out = re.sub(r"\{\{([a-z0-9_.]+)\}\}", fact, out)
    out = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", out)
    out = re.sub(r"`(.+?)`", r"<code>\1</code>", out)

    def link(m):
        label, target = m.group(1), m.group(2)
        if target.startswith("/"):
            return f'<a href="{target}{"&" if "?" in target else "?"}lang={L.code}">{label}</a>'
        return f'<a href="{L.href(target)}">{label}</a>'

    return re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", link, out)


def render(L, lines, facts, tables):
    out, para, bullets, rows_, formula = [], [], [], [], None

    def flush():
        if para:
            out.append(f"<p>{inline(L, ' '.join(para), facts, tables)}</p>")
            para.clear()
        if bullets:
            out.append("<ul>" + "".join(f"<li>{inline(L, b, facts, tables)}</li>" for b in bullets) + "</ul>")
            bullets.clear()
        if rows_:
            head, *body = rows_
            out.append('<div class="scroll"><table><thead><tr>' + "".join(f"<th>{inline(L, c, facts, tables)}</th>" for c in head)
                       + "</tr></thead><tbody>" + "".join("<tr>" + "".join(f"<td>{inline(L, c, facts, tables)}</td>" for c in r) + "</tr>" for r in body)
                       + "</tbody></table></div>")
            rows_.clear()

    for line in lines:
        if formula is not None:
            if line.strip() == "```":
                out.append(f'<pre class="formula">{inline(L, chr(10).join(formula), facts, tables)}</pre>')
                formula = None
            else:
                formula.append(line)
            continue
        s = line.strip()
        if s == "```":
            flush()
            formula = []
        elif not s:
            flush()
        elif s.startswith("{{table:") and s.endswith("}}"):
            flush()
            name = s[8:-2]
            if name not in tables:
                raise SystemExit(f"text: no table {s}")
            out.append(tables[name](L))
        elif s.startswith("## "):
            flush()
            out.append(f"<h2>{inline(L, s[3:], facts, tables)}</h2>")
        elif s.startswith("- "):
            if para or rows_:
                flush()
            bullets.append(s[2:])
        elif s.startswith("|"):
            if para or bullets:
                flush()
            cells = [c.strip() for c in s.strip("|").split("|")]
            if not all(re.fullmatch(r":?-+:?", c) for c in cells):
                rows_.append(cells)
        else:
            if bullets or rows_:
                flush()
            para.append(s)
    flush()
    return "".join(out)


def read(slug):
    """{lang: (title, meta, lines)} of one text page."""
    pages = {}
    for lang in ("en", "uk"):
        path = TEXT / f"{slug}.{lang}.md"
        lines = path.read_text(encoding="utf-8").splitlines()
        if not lines or not lines[0].startswith("# "):
            raise SystemExit(f"{path}: the first line is not '# Title'")
        title, meta, i = lines[0][2:].strip(), {}, 1
        while i < len(lines) and re.match(r"^[a-z]+: ", lines[i]):
            k, v = lines[i].split(": ", 1)
            meta[k] = v.strip()
            i += 1
        pages[lang] = (title, meta, lines[i:])
    return pages


def slugs():
    en = {p.name[:-len(".en.md")] for p in TEXT.glob("*.en.md")}
    uk = {p.name[:-len(".uk.md")] for p in TEXT.glob("*.uk.md")}
    if en != uk:
        raise SystemExit(f"text: pages without both languages: {sorted(en ^ uk)}")
    return sorted(en)


def add_all(site, facts, tables):
    for slug in slugs():
        pages = read(slug)
        title = {lang: pages[lang][0] for lang in pages}
        summary = {lang: pages[lang][1].get("summary", "") for lang in pages}
        section = pages["en"][1].get("section", "mechanics")
        site.add(slug, title, [], lambda L, p=pages: render(L, p[L.code][2], facts, tables),
                 section=section, summary=summary, written=True)

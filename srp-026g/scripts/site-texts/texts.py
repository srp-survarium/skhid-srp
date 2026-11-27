"""The texts of a page built at a release (`release/build-site`), for the web server to fill in
the reader's language.

A built page is one copy for every language: where it says something, its markup has a place,
`⟦<n>⟧`, and the page's table (`<page>.texts.json`, written beside it) says what goes there:

    {"word": "Fire rate"}                         the site's text (words/<code>.tsv), or in a context:
    {"word": "Reset", "context": "skills"}
    {"word": "Enemy at {m} m", "places": {"m": "5"}}             with its places filled
    {"one": "{n} point", "other": "{n} points", "places": {"n": "5"}}    by the count {n}
    {"game": {"en": "AK-74", "uk": "АК-74", ...}}  the game's text in each language (a build reads
                                                  the game's dictionaries); {"html": ...} markup
                                                  the build made of them

The server writes each text escaped for HTML, and for a place `⟦<n>j⟧` (inside a JSON string of
an attribute: `attribute_json`) as a JSON string's content, then escaped for HTML. A page's
`⟦words⟧` (in its `<script type="application/json" id="page-words">`) gets the words its script
asks for (`T("…")`, `words.js`), and `⟦lang⟧` the language's code. The web server's `src/built.rs` does all this; its words test
asks every language's file for every text of every table.

    texts = Texts()
    f'<label>{texts.t("Fire rate")}</label>'
    f'<div data-setup="{texts.attribute_json(rows)}">'
    texts.write(out / "weapons.texts.json")
"""
import html
import json
import re
from pathlib import Path

# The game's languages, as the site codes them, in its order (web-server Lang::ALL), and the
# folders of their dictionaries in the game's archive (resources_db::server::LOCALIZATIONS).
LANGUAGES = {"en": "english", "uk": "ukrainian", "ru": "russian", "de": "german", "pl": "polish",
             "sr": "serbian", "cs": "czech", "hu": "hungarian", "fi": "finnish", "be": "belarusian",
             "hr": "croatian", "pt": "portuguese"}
CODES = tuple(LANGUAGES)
# Each language's dictionary in the game's archive.
DICTIONARIES = [f"localization/{folder}/localization" for folder in LANGUAGES.values()]
# The words of a page's script, and the reader's language's code (for `?lang=` in a link).
WORDS, LANG = "⟦words⟧", "⟦lang⟧"
PLACE = re.compile(r"⟦(\d+)⟧")


class Texts:
    """A built page's table: each text once, by what it is."""

    def __init__(self):
        self.entries, self.index = [], {}

    def _place(self, entry):
        key = json.dumps(entry, sort_keys=True, ensure_ascii=False)
        if key not in self.index:
            self.index[key] = len(self.entries)
            self.entries.append(entry)
        return f"⟦{self.index[key]}⟧"

    def t(self, english, context=""):
        """The site's text `english` (its row in words/<code>.tsv)."""
        assert english == english.strip() and english, f"{english!r}: a text has no space around it"
        return self._place({"word": english, **({"context": context} if context else {})})

    def tf(self, english, **places):
        """The site's text `english` with its `{places}` filled: a translation may order them."""
        assert places and all(f"{{{name}}}" in english for name in places), f"{english!r}: {places}"
        return self._place({"word": english, "places": {k: str(v) for k, v in places.items()}})

    def plural(self, one, other, n):
        """`n` of something: `one` (with `{n}`) for one, `other` for more, as each language counts."""
        assert "{n}" in one and "{n}" in other
        return self._place({"one": one, "other": other, "places": {"n": str(n)}})

    def game(self, by_code):
        """The game's text in each language (`by_code`: a code each, English at least): the
        server writes it escaped."""
        return self._place({"game": complete(by_code)})

    def markup(self, by_code):
        """HTML the build made in each language (of the game's texts, escaped), as it is;
        nothing where it is empty in every one."""
        if not any(by_code.values()):
            return ""
        return self._place({"html": complete(by_code)})

    def attribute_json(self, value):
        """`value` as JSON for an attribute: escaped for HTML, its places JSON's."""
        return PLACE.sub(r"⟦\1j⟧", html.escape(json.dumps(value, ensure_ascii=False)))

    def write(self, path):
        Path(path).write_text(json.dumps(self.entries, ensure_ascii=False, indent=0) + "\n")


def complete(by_code):
    """A text in every language: English where one has none."""
    assert by_code.get("en"), f"{by_code}: no English"
    return {code: by_code.get(code) or by_code["en"] for code in CODES}


class English:
    """`Texts`' calls as English text, for a build's tests (and a page that is English only)."""

    def t(self, english, context=""):
        return english

    def tf(self, english, **places):
        return re.sub(r"\{(\w+)\}", lambda m: str(places.get(m.group(1), m.group(0))), english)

    def plural(self, one, other, n):
        return (one if n == 1 else other).replace("{n}", str(n))

    def game(self, by_code):
        return by_code["en"]

    def markup(self, by_code):
        return by_code["en"]

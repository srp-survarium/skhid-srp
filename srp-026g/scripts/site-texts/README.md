# The built pages' texts

The pages `release/build-site` builds (the wiki's weapons and simulators, and the others as they
move here) are one copy for every language. What they say comes from two places:

- the site's own words: `servers/web-server/words/<code>.tsv`, as for every other page of the site.
  A builder writes `texts.t("Fire rate")` (`texts.py`), which leaves a place `⟦n⟧` in the page and
  the text in the page's table (`<page>.texts.json`); the web server fills it in the reader's
  language at each request (`servers/web-server/src/built.rs`). A change of a row shows at once:
  nothing is rebuilt for it.
- the game's texts (weapon, round, skill and modification names): read by the build from the
  game's dictionaries in every language (`DICTIONARIES`) and kept in the table, each language's.

A page's script says its words with `T("…")`, `Tc("context", "…")` and `Tf("… {n}", {n})`
(`words.js`, put first in each script): their English as a literal, which the server finds in the
script and gives the page as `page-words`. The words test (`servers/web-server/src/words.rs`) asks
every language's file for every text of every table and script, so a text without its rows fails it.

A text with a number in it is a template (`texts.tf("Enemy at {m} m", m=5)`), so a language orders
it as it reads best; a count that changes its noun is `texts.plural("{n} point", "{n} points", n)`,
whose rows are the contexts `one`, `few` and `many` (a language with two forms writes its plural in
both of the last two).

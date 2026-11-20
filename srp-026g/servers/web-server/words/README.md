# The site's own words

The web server's pages are written in English in the code: `lang.t("Daily leaders")`. What a page
says in another language is here, one file a language (`<code>.tsv`, the codes of `Lang::code`:
the game's eleven languages but English, whose words are the code's own).

A row is

```
<the English text, exactly as in the code><TAB><the same in this language>[<TAB><context>]
```

- `#` starts a comment line; empty lines are skipped.
- `\n`, `\t` and `\\` are a line break, a tab and a backslash, in both texts.
- `{name}` and `{0}` are places the server fills (`lang.tf`): a translation keeps each of them, in
  whatever order reads best. HTML in a text (`<b>`, `&mdash;`) stays as it is.
- A context tells apart two meanings of one English text (`lang.tc("button", "Ban")`: the button,
  not the noun).
- A text a file lacks is shown in English. Ukrainian lacks none: the tests (`words.rs`) refuse a
  text of the code that `uk.tsv` has not, and in every file a row the code no longer has, an empty
  translation, or one with other `{places}` than its English.

To translate a language: copy the rows of `uk.tsv`, keep the first column, write the second.

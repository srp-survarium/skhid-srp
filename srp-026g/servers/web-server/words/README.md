# The site's own words

The web server's pages are written in English in the code: `lang.t("Daily leaders")`. What a page
says in another language is here, one file a language (`<code>.tsv`, the codes of `Lang::code`:
the game's twelve languages but English, whose words are the code's own).

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
- Every language has every text the code asks for. The test (`words.rs`,
  `every_language_has_every_text`) names, file by file, each text a file lacks, as the row to add with
  its translation left to write, and each row the code no longer has, an empty translation or one with
  other `{places}` than its English. A text meant to stay English in a language has its row, with the
  English as its translation.

To translate a language: copy the rows of `uk.tsv`, keep the first column, write the second.

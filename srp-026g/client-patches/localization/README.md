# Game localization (0.26g)

All nine native game dictionaries live here as UTF-8 TSVs: English, Ukrainian,
Russian, German, Polish, Serbian Latin, Czech, Hungarian and Finnish. Every file
has the same 2,315 keys, sorted by full key name. These are the build sources,
including unchanged weapon names and key labels; there are no separate keep lists
or per-language dictionary patches. The engine's `original` dictionary is built
from `russian.tsv`.

## Edit text or add an item

1. Edit the appropriate `<language>.tsv`. For a new item, add its name and description
   keys to **every** TSV, in key order, and use those keys in the item's config.
2. Preserve placeholders (`{USE}`, `%s`, `%d`, etc.) and markup. Review every
   translation when the English meaning changes; format checks cannot detect stale wording.
3. From the repository root, run this single command (Python 3, no dependencies or game assets):

   ```sh
   python3 srp-026g/client-patches/check_localization_026g.py
   ```

The first row is `key<TAB>text`. Each following row contains the **full** key,
a literal tab, and its text. The only escapes are `\n` (newline), `\r`, `\t`,
and `\\` (literal backslash). Do not trim leading/trailing spaces in text.
Quotes are literal text, not TSV quoting. Empty text is permitted only for the
four unused `st_turn_*_action` control labels. Duplicate keys, unregistered or
missing languages, missing/extra keys, empty translations, malformed UTF-8,
NULs, changed placeholders/markup, and text exceeding the native 511-byte limit
fail the check. Every dictionary is also compiled and decoded to verify it
round-trips exactly. Unchanged text is informational: model names and keycaps
can legitimately match English.

The same command checks the custom Flash language tables, language constants,
native label detectors, content edit key references, and the final catalog build step.
Custom menu sentences still live in ActionScript; those are separate from native
item dictionaries. Their existing column order is English, Ukrainian, Russian,
German, Polish, Serbian, Czech, Hungarian, Finnish. Extend those tables when
adding a language. Some menus detect the language from the localized Friends,
Cancel or registration labels, so changes to those labels must also update the
Flash detectors. Regression tests for the checker and build integration run with:

```sh
python3 -m unittest discover -s srp-026g/client-patches -p test_localization.py
```

## Content build

`release/content.tsv` ends with `scripts/content-edit/localization-1.json`.
It compiles **all** dictionaries from these TSVs through
`build_localization_026g.py`, after the content packs and gameplay/UI edits.
Dictionary text previously held in edit lists has moved here. Add future
text here as well; keep gameplay values and UI key mappings in their edit lists.

The compiler rejects missing translations and any source dictionary key absent
from the TSV catalog, so a new content pack cannot silently lose its text.
A content pack that changes the text of an existing key needs the same change
here: the final compile writes the TSV text over the pack's.
It also requires every native Flash UI mapping key to exist. Binary dictionaries
use the engine's CRC key order, while TSVs use readable alphabetical order.
Build with `release/build-content` as usual; never assemble a resource patch manually.

To additionally audit extracted compiled resources (a directory tree, dbgrab's
flat filenames, or UTF-8 JSON `{"strings": {...}}` dictionaries):

```sh
python3 srp-026g/client-patches/check_localization_026g.py /path/to/extracted-resources
python3 srp-026g/client-patches/check_localization_026g.py --json
```

Include `localization/flash_movies_localizations.options` to check UI references;
its absence is reported. All registered languages are required by default;
`--require LANGUAGE` selects an explicit subset for a resource audit.
`--catalog` and `--flash-source` select alternate source directories.
Exit codes: 0 clean, 1 coverage/content defects, 2 invalid input.

The migration preserves the existing translations and the reviewed Serbian wording.
It fills the two missing Polish and two missing Ukrainian entries, repairs the
German `st_reach_level` closing tag and English quest text's invalid encoding,
and supplies the nine stock UI keys previously absent from English.
The source still contains unfinished item descriptions (`ОПИСАНИЕ НЕ ГОТОВО!!!`);
the added languages translate these as unavailable-description notices.
Key coverage does not mean those descriptions have been authored.

## Select or add a language

Players choose the language in the game's Settings (Gameplay, Language), which
lists every registered language by its own name and keeps the choice in user.cfg;
it applies at the next start (exe patch 79, `../../docs/notes/language-option.md`).
`-localization=czech` on the command line (or `LANGUAGE=czech` in
`play_skhid.bat` / `localization=czech` in `play_skhid.sh`) starts the game in it
and saves it as the choice. The overlay builder accepts `--localization=czech`
and the other resource names (the scripts' default; none by default). The
launcher's dropdown keeps its five choices for its own texts. Website and
launcher UI translations are separate.

To add a language, register its resource name, Flash constant and own name in
`LANGUAGES` in `localization_026g.py`, run `python3 localization_026g.py --write`
(the exe's table, `languages_026g.rs`), add its complete TSV here (with the
Settings' two texts `st_srp_language_option` and `st_srp_language_restart`), and extend custom Flash
language tables/detectors and their runtime checks. The content compiler
creates its resource directory automatically. Existing languages retain their
support files; new languages inherit English EULA and profanity data, which
are support resources, not translated text.

The rebuilt lobby's runtime check exercises language detection, the four added
languages' room/map/mode labels, chat tab widths and embedded Calibri glyphs:

```sh
srp-026g/scripts/localization-check/check.sh /path/to/lobby_menu.swf
```

It uses the existing Java/Flex configuration and Nix's Ruffle on a private virtual
display. It does not start the game or connect to a server. Some legacy font
subsets have fewer glyphs; this does not verify every game screen. Native-speaker
review and an in-game layout pass remain useful. Nickname/clan character
restrictions and numeric chat/tutorial language codes are separate from dictionary
selection and are unchanged.

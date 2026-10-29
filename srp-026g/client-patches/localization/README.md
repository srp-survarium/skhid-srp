# Game localization (0.26g)

All ten native game dictionaries live here as UTF-8 TSVs: English, Ukrainian,
Russian, German, Polish, Serbian Latin, Czech, Hungarian, Finnish and Belarusian
(Cyrillic, standard orthography). Every file
has the same 2,726 keys, sorted by full key name. These are the build sources,
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
NULs, changed placeholders/markup, Cyrillic in a Latin language, Russian or
Ukrainian letters (и, щ, ъ, ї, є, ґ) in Belarusian, and text exceeding the native
511-byte limit fail the check. Every dictionary is also compiled and decoded to
verify it round-trips exactly. Unchanged text is informational: model names and
keycaps can legitimately match English.

The same command checks the Flash menus' text keys (below), content edit key
references, and the final catalog build step. Regression tests for the checker and
build integration run with:

```sh
python3 -m unittest discover -s srp-026g/client-patches -p test_localization.py
```

## Flash menus' own texts

srp's own menu texts (the rooms, the Matches window, Player vs Bots, the replay
prompt, the gold exchange, loadouts, the reroll rule, the search box, the login hint,
the weapon cards' details, the weight tooltip) are dictionary keys like any other text:
`st_srp_<menu>_<name>` (`st_srp_rooms_*`, `st_srp_matches_*`, `st_srp_pvb_*`,
`st_srp_replay_*`, `st_srp_exchange_*`, `st_srp_loadout_*`, `st_srp_login_*`,
`st_srp_weapon_*`, `st_srp_weight_*`; a value with its unit `st_srp_unit_*`, as the
stock cards write it: `%1$ss`, `%1$sс`; maps, modes and weathers `st_srp_map_*`,
`st_srp_mode_*`, `st_srp_weather_*`). No movie holds language text, and adding or
changing a language never touches an `.as` file.

- **How a movie gets them.** The exe hands the lobby and login movies their texts
  through `set_localization_data`, by the movie's table of
  `localization/flash_movies_localizations.options` (UI key to dictionary key). The
  content compile (`localization-1.json`) adds every `st_srp_` key to the lobby's and
  the login's tables under its own name (`FLASH_MOVIES`, `flash_rows` in
  `localization_026g.py`): a new key needs no mapping of its own.
- **In ActionScript:** `LocalizationData.getName("st_srp_rooms_join")`, or
  `LocalizationData.format("st_srp_rooms_switch_who", name, players)` for a text
  with values. Write the key in full in the source: the check finds it there.
- **Values** are `%1$s`, `%2$s`, ... in the text, in any order a language needs; every
  language keeps the same ones. Never `{name}`: the game's localizer (0x1FEF90) drops
  any `{...}` that is not an action's key from the texts it hands a movie.
- **The check** fails a key a movie names that the dictionaries lack, an `st_srp_` key
  named by another movie (it is not given srp's texts), and an `st_srp_` text with
  braces. With a compiled resource tree it also fails an `st_srp_` key that the lobby's
  or the login's mapping does not map to itself.
- **Fit and terms.** A label, header or button text fits its place on one line wherever the
  English does; prefer a shorter natural wording to an abbreviation, and name things with the
  stock dictionary's own words of that language (recoil, sway, energy/stamina, reload...). The
  weapon card's and the rooms' Ruffle checks measure every language
  (`scripts/weapon-details/`, `scripts/rooms-start/`; their `OVERFLOW`/`OVERLAP` lines).
- The main menu's Language choice (`SrpLanguage.as`) gets its two texts from the exe's
  answer, which reads them from the dictionary (patch 79).
- The inventory search's item-kind words (`InventoryList.SEARCH_KINDS`) are not shown
  text: the words of every language match whatever the game's language, so they stay
  a word list in the code.

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
`LANGUAGES` in `localization_026g.py` (and in `CYRILLIC` for a Cyrillic one), run
`python3 localization_026g.py --write` (the exe's table, `languages_026g.rs`), add its
complete TSV here (with every `st_srp_` text, the Settings' two
`st_srp_language_option` and `st_srp_language_restart` among them). No Flash source
changes. The content compiler
creates its resource directory automatically. Existing languages retain their
support files; new languages inherit English EULA and profanity data, which
are support resources, not translated text.

The rebuilt lobby's runtime check gives the movie every language's texts as the exe
does (`../../scripts/localization-check/harness_text.py`, from these TSVs) and checks
its lookups by number (maps, modes, weathers, room codes), the room's chat lines, every
text's placeholders, the room chat tab's width and the embedded Calibri's glyphs of
every srp text (run it under `nix-shell -p python3`):

```sh
srp-026g/scripts/localization-check/check.sh /path/to/lobby_menu.swf
```

It uses the existing Java/Flex configuration and Nix's Ruffle on a private virtual
display. It does not start the game or connect to a server. Some legacy font
subsets have fewer glyphs; this does not verify every game screen. Native-speaker
review and an in-game layout pass remain useful. Nickname/clan character
restrictions and numeric chat/tutorial language codes are separate from dictionary
selection and are unchanged.

Text encoding: the dictionaries are UTF-8 (the stock Russian and Ukrainian ones
too), read by the engine as bytes and drawn by Scaleform, so Belarusian ў/Ў
(U+045E/U+040E) needs nothing special. Every embedded font of the lobby, login,
main menu, HUD and replay movies that has Cyrillic also has ў/Ў and і/І. At
start-up (0xE3C10) the exe sets the C runtime's collation to the system code page
(`setlocale(LC_COLLATE, ".ACP")`), then the whole C locale only for
original/russian (1251), english (1252) and polish (1250). Belarusian, like
Ukrainian and the added languages, keeps the "C" character types, which leave
UTF-8 bytes alone.

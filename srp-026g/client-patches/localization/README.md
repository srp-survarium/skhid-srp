# Game localization (0.26g)

Serbian uses Latin script (`sr-Latn`) and the resource directory `serbian`.
Start the game with `-localization=serbian`, or set `LANGUAGE=serbian` in
`play_skhid.bat` / `localization=serbian` in `play_skhid.sh`.
The overlay builder accepts `--localization=serbian`. The original Windows
launcher's language dropdown still has its five original choices; use the game
script for Serbian. The website and launcher UI are separate translations.

The native dictionary loader accepts a language directory without a new executable
patch. The harder parts are translating all current content and updating custom
Flash text tables. Their column order is English, Ukrainian, Russian, German,
Polish, Serbian; `Language.SR` is 5. Some detect the language from the localized
Friends or Cancel label, and the login hint uses the registration label.

## Checking coverage

From the repository root, with Python 3 and no third-party packages:

```sh
python3 -m unittest discover -s srp-026g/client-patches -p test_localization.py
python3 srp-026g/client-patches/check_localization_026g.py /path/to/extracted-resources --require serbian
python3 srp-026g/client-patches/check_localization_026g.py /path/to/extracted-resources --json > report.json
```

Supply the effective resources (stock plus every published patch, in order).
The checker accepts a resource tree, dbgrab's flat filenames, or UTF-8 JSON
`{"strings": {...}}` dictionaries. Include
`localization/flash_movies_localizations.options` to check UI references too;
its absence is explicitly reported. `--require` can be repeated to detect a
missing language folder. Every discovered dictionary is compared by key, not
just entry count. Exit codes: 0 clean, 1 coverage/content defects, 2 invalid input.

Checks include missing keys, unexpectedly empty text, duplicate keys, invalid
UTF-8, game substitutions, printf arguments (including their order), markup,
UI references absent even from English, and the custom Flash language columns.
Extra keys and unchanged text are reported for review; model names and keycaps
can legitimately be unchanged. `--flash-source` selects another checkout's
Flash tree. The Flash check recognizes the existing language-table convention;
new ways of storing text need corresponding checks.

The stock/current dictionaries have pre-existing defects: Polish lacks
`st_notes_if_you_read` and `st_notes_reward`; Ukrainian lacks
`st_best_support_healed` and `st_best_support_saved`. German has malformed markup
in `st_reach_level`; English has a non-UTF-8 byte in
`st_renaissance_kill_enemies_descr`. Nine UI keys are absent from English itself.
The full audit deliberately fails on these rather than hiding them in an
allowlist. Serbian supplies those UI keys and has no dictionary coverage errors.
The source also contains 111 unfinished item descriptions (`ОПИСАНИЕ НЕ ГОТОВО!!!`).
Serbian preserves their meaning as `Opis još nije dostupan.`; key coverage does
not mean those descriptions have been authored. No existing description was
replaced with a placeholder.

For the rebuilt lobby movie, the runtime check exercises language detection,
Serbian room/map/mode labels and the embedded Calibri glyphs:

```sh
srp-026g/scripts/localization-check/check.sh /path/to/lobby_menu.swf
```

It uses the existing Java/Flex tool configuration and Nix's Ruffle on a private
virtual display. It does not start the game or connect to a game server.

## Editing or adding a language

`serbian.tsv` contains translations. Keys omit `st_`; `@full_key` denotes a
key without that prefix. A literal `\n` means a line break. Preserve substitutions
and markup. `serbian_keep.txt` explicitly names unchanged model identifiers,
keycaps, units and empty controls. There is no implicit English fallback.

`scripts/content-edit/serbian-1.json` copies the English support resources and
compiles the dictionary through `build_localization_026g.py`. The compiler
rejects missing or unknown keys, duplicate definitions, broken substitutions,
mixed Cyrillic text, and strings exceeding the native 511-byte text limit. It
writes CRC-sorted binary records and verifies the UTF-8 round trip.

Keep the translation row at the end of `release/content.tsv`, after edits that
add English keys. A newly added key stops the content build until it has a
translation or an explicit keep entry. Changes to the meaning of an existing
English string still need translator review; key/format checks cannot detect
a stale translation. For another language, add its table, keep list and content
edit, register it in `make_client_026g.py`, extend all custom Flash language
tables/detectors and update their checks. Build with `release/build-content`
as usual; never assemble a resource patch manually.

The main Calibri fonts include Serbian Latin accents. Some legacy font subsets
do not; the runtime check covers the rooms' Calibri font, not every game screen.
Native-speaker review and an in-game layout pass remain necessary. The English
EULA and profanity data are copied as support resources, not translated.
Legacy nickname/clan character restrictions and numeric chat/tutorial language
codes are separate from dictionary selection; this change does not extend them.

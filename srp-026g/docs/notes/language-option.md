# Language in Settings

The owner (2026-10-29): a Language choice in the game's Settings, every registered language by its
own name; "explicit flag simply sets the current language, it can later be changed in settings";
"why registry? I feel user.cfg suits better". Exe patch 79 (`client-patches/rust/patches/src/language.rs`),
the main menu movie (`flash/main_menu/src/SrpLanguage.as`).

## The choice

- **Where:** Settings, Gameplay tab, bottom right: the heading "Language", a list of every registered
  language by its own name (English, Українська, Русский, Deutsch, Polski, Srpski, Čeština, Magyar,
  Suomi, ...), the saved one selected (in Ruffle: `../images/language-option.png`).
- **A pick is saved at once** (no Apply): user.cfg is written at the pick, not only when the game
  closes.
- **Applies after restart:** the note "Applies after restart." shows while the saved language is not
  the running one. No live switch: the engine reads its language once, before the dictionary, the
  Flash texts, the swear filter and the chat language load.
- **Stored in user.cfg** (`Documents\survarium\user.cfg`), as the console variable `localization`
  (`localization czech`): the engine's own saved string variable, saved and read as every other
  setting is. A line the engine does not know (an older game) is ignored.

## At start

The language, once, before the dictionary loads:

1. `-localization=<name>` on the command line, when it is a registered language: it is the language
   **and** it is saved as the choice (Settings can change it afterwards);
2. else the saved choice (user.cfg), when it is a registered language;
3. else the launcher's menu language (`HKCU\Software\Vostok Games\Survarium`, `language`: EN, RU, UA,
   DE, PL), so a player who never opened Settings keeps the language he had;
4. else English.

- Only registered languages are taken (`LANGUAGES` in `client-patches/localization_026g.py`); any
  other name (a typo, the engine's `original`) is passed over to the next source.
- The resolved language is saved as the choice: the next start without a flag starts in it.
- The C locale (code page) is set as a start with that flag sets it (core::preinitialize's table).

## The start paths

- **Launcher (PLAY):** passes no `-localization=`. Its language menu changes the launcher's texts.
  The game takes it once, at its first start with patch 79 (rule 3), and saves it; after that the
  game's language is changed in Settings only.
- **Watch replay** (`updater_skhid.exe replay`): no `-localization=`; the replay plays in the game's
  saved language.
- **`play_skhid.sh` / `play_skhid.bat`:** no language by default (`localization=`, `set LANGUAGE=`).
  Set there, it is passed as the flag (rule 1). The release no longer bakes one in.

## One list

- The languages, their resource names and their own names are `LANGUAGES` in
  `client-patches/localization_026g.py`. `python3 localization_026g.py --write` generates the exe's
  table (`client-patches/languages_026g.rs`); `check_localization_026g.py` fails when it is stale.
- The movie lists what the exe answers (`root.srp_language`): label, note, saved, running, then each
  language's name and own name. No list in the movie.
- A new language: its registry row (with its own name), its complete TSV (the two Settings texts
  `st_srp_language_option`, `st_srp_language_restart` included), `--write`.

## Known limits

- The crash reporter's log transcoding still guesses the code page from the command line
  (`client/bugtrap/src/logs.rs`): without a flag it assumes cp1251. Only non-UTF-8 log lines (audio
  device names) of a player whose language has another code page are affected.
- Checked: Unicorn (`check_language_026g.py`), Rust tests, the movie in Ruffle
  (`scripts/language-option/check.sh`). Not yet run in the game.

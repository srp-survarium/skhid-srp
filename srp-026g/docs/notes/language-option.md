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
3. else a language picked in the launcher's menu (`HKCU\Software\Vostok Games\Survarium`, `language`:
   RU, UA, DE, PL), so a player who never opened Settings keeps the language he had. EN (0) is passed
   over: the launcher's first start writes it when nothing was chosen (`launcher.md` section 10.2;
   Skhid starts it without `-language=`), so it is no choice;
4. else the language of the player's Windows user locale (the owner, 2026-10-29: "set user.cfg with a
   current user locale if it is not set before the engine starts; if they have different locale --
   they will get english one"): `GetUserDefaultLCID`, matched by its primary language alone (any
   country or script: German of Austria is German). Serbian, Croatian and Bosnian (primary 0x1A, Latin
   or Cyrillic) get Serbian, the Latin one we have;
5. else English.

- Only registered languages are taken (`LANGUAGES` in `client-patches/localization_026g.py`); any
  other name (a typo, the engine's `original`) is passed over to the next source.
- **The user locale, not the UI language.** The owner said locale; the game imports
  `GetUserDefaultLCID` (kernel32), and no `GetUserDefaultUILanguage`, which would need a looked-up
  function the engine-function library has no kind for. On Windows the two differ only for a player
  whose Region format is not his display language (an English Windows set to Russian formats starts
  in Russian). Under Wine both are the Unix locale's LC_MESSAGES as glibc resolves it (`LC_ALL`, else
  `LC_MESSAGES`, else `LANG`; `LANGUAGE` is not read; Wine's `ntdll/unix/env.c` `init_locale`). A
  locale the system has not generated makes glibc answer `C`: then only an `LC_ALL` value is still
  taken, else English. `LANG=ru_RU.UTF-8 bash play_skhid.sh` starts a new player in Russian.
- The resolved language is saved as the choice: the next start without a flag starts in it.
- The C locale (code page) is set as a start with that flag sets it (core::preinitialize's table).

## The start paths

- **Launcher (PLAY):** passes no `-localization=`. Its language menu changes the launcher's texts.
  The game takes a non-English pick once, at its first start with patch 79 (rule 3), and saves it;
  after that the game's language is changed in Settings only.
- **Watch replay** (`updater_skhid.exe replay`): no `-localization=`; the replay plays in the game's
  saved language.
- **`play_skhid.sh` / `play_skhid.bat`:** pass no language; a hand-typed `-localization=` on a
  command line is still rule 1.

## One list

- The languages, their resource names, their own names and their Windows primary language ids are `LANGUAGES` in
  `client-patches/localization_026g.py`. `python3 localization_026g.py --write` generates the exe's
  table (`client-patches/languages_026g.rs`); `check_localization_026g.py` fails when it is stale.
- The movie lists what the exe answers (`root.srp_language`): label, note, saved, running, then each
  language's name and own name. No list in the movie.
- A new language: its registry row (with its own name and primary language id), its complete TSV (the two Settings texts
  `st_srp_language_option`, `st_srp_language_restart` included), `--write`.

## Known limits

- The crash reporter's log transcoding still guesses the code page from the command line
  (`client/bugtrap/src/logs.rs`): without a flag it assumes cp1251. Only non-UTF-8 log lines (audio
  device names) of a player whose language has another code page are affected.
- Checked: Unicorn (`check_language_026g.py`), Rust tests, the movie in Ruffle
  (`scripts/language-option/check.sh`). Not yet run in the game. The test for rule 4: a prefix
  whose user.cfg has no `localization` line and whose registry `language` is 0 or missing, started
  with `LANG=ru_RU.UTF-8` (a generated locale), comes up in Russian and its user.cfg then says
  `localization russian`; again with `LANG=ja_JP.UTF-8` and the line removed, English.

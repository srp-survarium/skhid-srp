# Account names and clan abbreviations: their characters

The rules the code and the tests follow. Code: `session::v026g::names` (`base_char`,
`name_char`, `shown_name`, `made_safe`, `unique`, `same_name`), `session` `lib.rs`
(`SessionStore::find_or_create`, `name_taken`, `make_names_safe` on loading),
`v026g::Accounts::take_names` (srp's start), `lobby-server` `dispatch.rs::name_refusal` and
`shop.rs::nickname_refusal`, `session::v026g::clans::name_fault`. Why: the movies put a
player's name and clan abbreviation into the HTML of other players' chat unescaped (only the
line's text is: `styledChar`); see `relay_trust.md` section 4 (gap 6).

- **Where a name is shown:** the lobby chat (`LobbyChat.as::get_styled_text`: `"[" + abbr +
  "] "` and `<a href='event:name:<name>: '><name>: </a>`), the match chat (`GameChat.as`),
  friend lists and search results (0xA3), important messages (0xA4), squad and clan info, the
  rankings and results, the match's static info (0xE4), the web pages (escaped there).
- **Letters, one set for every name (`base_char`):** the game's own regexes'
  (`gameplay/clans/clan_name.options`): Latin letters, digits, Cyrillic A..ya, the Ukrainian
  and German letters (ye, i, yi; a, o, u umlaut, sharp s). Added: Cyrillic yo and Ukrainian
  ghe (both cases), letters real players use. Account names add `-`, `_`, `+`; clan names add
  `#`, `-` and the blank.
- **A chosen name (rename, `49 03` / `49 04`):** 3 characters or more, no blank, at most the
  client's 63 bytes, the letters above and `-`, `_`, `+` only. Not taken: the regex's blank
  inside a name (a rename refuses blanks, as before), its `|` (the author's separator), and
  **no `@`** (the owner, 2026-10-22). Anything else: `WrongSymbol`, the client's own reason
  in the rename window. The player's own name, case aside, passes these rules (one saved
  before them, with a `.` or a blank, is its to keep); it is still `Taken` if another account
  has it.
- **A new account (first sign-in):** the login may hold anything the login server takes (as
  before: no control characters). The name it is shown by is the login's part before its first
  `@` (as before), with every character a chosen name may not have made `_`: `john.doe@x`
  shows as `john_doe`. Nothing is refused, so no sign-up fails over it.
- **One name, one account:** a new account's name is no other account's name or login, case
  aside (a rename's rule). If it is, `_2`, `_3`, ... is added (the name cut so the number fits
  the 63 bytes): `john.doe@x` with a `john_doe` already there is `john_doe_2`. A name with no
  letter or digit left (a login of Chinese letters -> `___`) is `player_<id as 8 hex digits>`, as an empty one
  always was. [choice] The names are compared with no lock held: two sign-ups of one name at
  the same instant may still both get it.
- **A name saved before (on loading):** only what no name may show is made `_`: `<`, `>`,
  `&`, `"`, `'` (the end of the name's link), `:` (the link's field separator:
  `clickLinkHandler` splits on it, so the player menu would act on another name), `@`, control
  characters, and the invisible ones that turn or hide text (Unicode format characters: the
  right-to-left override, zero-width spaces and joiners, the isolates, the byte-order mark, the
  tags; and the line and paragraph separators). A name merely outside the chosen set (a `.`
  from a login, a blank) stays as players know it.
  - The repaired name is made unique as a new one is. A clash with another account's name or
    login is logged as an error with both ids; every change as a warning; and saved. The
    sign-in is untouched: the login is not the name.
  - Names saved before names were unique (two `stepan@...` both `stepan`) are kept.
- **The two copies:** the store's name is the only one made or repaired. At srp's start the
  0.26g accounts take it (`take_names`, a count logged), so both are the same; a rename sets
  both, as before.
- **Clan name and abbreviation:** the clan characters above, now **without `@`** (the owner's
  rule for names). Existing clans keep theirs: only new names and abbreviations are checked.
- **Staff colours:** the lobby chat draws a line whose clan prefix starts with `[admin]` or
  `[VG]` in the staff colours (`LobbyChat.as`, case-sensitive). An abbreviation that reads
  as `admin` or `vg` (no case, look-alikes folded as the game's own replacement table does)
  is refused as `Taken`. `VG` is under the 3-character minimum anyway; `[` and `]` are no
  clan characters. An account's own name cannot colour a line (only the abbreviation is
  tested). An existing clan with such an abbreviation keeps it.
- **Not done:** escaping on the server. The client uses the name as a plain value too (the
  link's target, `_data.name == message.name`, a private line's receiver), so an escaped name
  would no longer match itself; with the characters above there is nothing to escape.
  Escaping in the lobby movie (and the staff colours from the server's `color`) needs a
  content release: raised with the owner apart.
- **Not looked at:** kit names (cut to 63 bytes, as before).

# Account names and clan abbreviations: their characters

The rules the code and the tests follow. Code: `session::v026g::names` (`name_char`,
`shown_name`, `made_safe`), `lobby-server` `dispatch.rs::name_refusal`,
`session::v026g::clans::name_fault`, the two `repair`s run on loading
(`persist::AccountRecord::repair`, `v026g::account::Account::repair`). Why: the movies put a
player's name and clan abbreviation into the HTML of other players' chat unescaped (only the
line's text is: `styledChar`); see `relay_trust.md` section 4 (gap 6).

- **Where a name is shown:** the lobby chat (`LobbyChat.as::get_styled_text`: `"[" + abbr +
  "] "` and `<a href='event:name:<name>: '><name>: </a>`), the match chat (`GameChat.as`),
  friend lists and search results (0xA3), important messages (0xA4), squad and clan info, the
  rankings and results, the match's static info (0xE4), the web pages (escaped there).
- **A chosen name (rename, `49 03` / `49 04`):** 3 characters or more, no blank, at most the
  client's 63 bytes, and only the game's own `account_name_regex` characters
  (`gameplay/clans/clan_name.options`): Latin letters, digits, Cyrillic A..ya, the Ukrainian
  and German letters (ye, i, yi; a, o, u umlaut, sharp s), `-`, `_`, `+`. Added to the game's
  set: Cyrillic yo and Ukrainian ghe (both cases), letters real players use. Not taken: the
  regex's blank inside a name (a rename refuses blanks, as before), its `|` (the author's
  separator), and **no `@`** (the owner, 2026-10-22). Anything else: `WrongSymbol`, the
  client's own reason in the rename window.
- **A new account (first sign-in):** the login may hold anything the login server takes (as
  before: no control characters). The name it is shown by is the login's part before its first
  `@` (as before), with every character a chosen name may not have made `_`: `john.doe@x`
  shows as `john_doe`. Nothing is refused, so no sign-up fails over it.
- **A name saved before (on loading):** only what no name may show is made `_`: `<`, `>`,
  `&`, `"`, `'` (the end of the name's link), `:` (the link's field separator:
  `clickLinkHandler` splits on it, so the player menu would act on another name), `@`, and
  control characters. A name merely outside the chosen set (a `.` from a login, a blank)
  stays as players know it. The change is logged per account (warn) and saved. The sign-in is
  untouched: the login is not the name.
- **Clan name and abbreviation:** the game's `clan_name_regex` characters as before
  (`clans::name_char`), now **without `@`** (the owner's rule for names; `#`, `-` and the
  blank inside a clan name stay). Existing clans keep theirs: only new names and
  abbreviations are checked.
- **Staff colours:** the lobby chat draws a line whose clan prefix starts with `[admin]` or
  `[VG]` in the staff colours (`LobbyChat.as`, case-sensitive). An abbreviation that reads
  as `admin` or `vg` (no case, look-alikes folded as the game's own replacement table does)
  is refused as `Taken`. `VG` is under the 3-character minimum anyway; `[` and `]` are no
  clan characters. An account's own name cannot colour a line (only the abbreviation is
  tested). An existing clan with such an abbreviation keeps it.
- **Not done:** escaping on the server. The client uses the name as a plain value too (the
  link's target, `_data.name == message.name`, a private line's receiver), so an escaped name
  would no longer match itself; with the characters above there is nothing to escape.
- **Not looked at:** kit names (cut to 63 bytes, as before).

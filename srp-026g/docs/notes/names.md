# Account names and clan abbreviations: their characters; sign-in by nickname

The rules the code and the tests follow. Code: `session::v026g::names` (`base_char`,
`name_char`, `name_refusal`, `shown_name`, `made_safe`, `unique`, `same_name`), `session`
`lib.rs` (`SessionStore::authenticate`, `find_or_create`, `name_taken`, `make_names_safe` on
loading), `v026g::Accounts::take_names`
(srp's start), `lobby-server` `shop.rs::nickname_refusal`, `login-server` `v026g.rs`
(`NICKNAME_REFUSED`), `session::v026g::clans::name_fault`, the edit list
`scripts/content-edit/login-nickname-1.json` and the login movie's `LoginMenu.as`
(`drawPlayerIcon`). Why: the movies put a player's name and clan abbreviation into the HTML of
other players' chat unescaped (only the line's text is: `styledChar`); see `relay_trust.md`
section 4 (gap 6). Sign-in by nickname: the owner, 2026-10-22, "in login menu, it should no
longer say email and instead say nickname".

- **Where a name is shown:** the lobby chat (`LobbyChat.as::get_styled_text`: `"[" + abbr +
  "] "` and `<a href='event:name:<name>: '><name>: </a>`), the match chat (`GameChat.as`),
  friend lists and search results (0xA3), important messages (0xA4), squad and clan info, the
  rankings and results, the match's static info (0xE4), the web pages (escaped there).
- **Letters, one set for every name (`base_char`):** the game's own regexes'
  (`gameplay/clans/clan_name.options`): Latin letters, digits, Cyrillic A..ya, the Ukrainian
  and German letters (ye, i, yi; a, o, u umlaut, sharp s). Added: Cyrillic yo and Ukrainian
  ghe (both cases), letters real players use. Account names add `-`, `_`, `+`, `.`; clan names add
  `#`, `-` and the blank.
- **A chosen name (rename, `49 03` / `49 04`):** 3 characters or more, no blank, at most the
  client's 63 bytes, the letters above and `-`, `_`, `+`, `.` only. The `.` is srp's (the
  owner, 2026-10-22: `john.doe` is a name); nothing a name feeds reads it (the chat link
  `event:name:<name>: ` splits on `:`, markup needs `<`, `>`, `&`, quotes; no file or URL path
  is made of a name). Not taken: the regex's blank inside a name (a rename refuses blanks, as
  before), its `|` (the author's separator), and **no `@`** (the owner, 2026-10-22). Anything
  else: `WrongSymbol`, the client's own reason in the rename window. The player's own name,
  case aside, passes these rules (one saved before them, with a blank, is its to keep); it is
  still `Taken` if another account has it.
- **The login screen** (edit list `login-nickname-1.json`, the six localization tables; the
  login movie):
  - the login field says Nickname, Spitzname, Nick, Никнейм (Russian and the "original"
    copy), Нікнейм (`st_login_default_text`, and `st_login_str`), not E-mail. The owner,
    2026-10-22: Нікнейм, Никнейм, Nickname; German and Polish take the game's own words for a
    player's nickname (`st_clan_name_is_not_valid`'s Spitzname, the battle notes' nicki);
  - its envelope icon (`SpriteMail`) is hidden and a player's head and shoulders drawn in its
    place (`LoginMenu.as::drawPlayerIcon`);
  - "Invalid e-mail or password" says nickname (`..._invalid_user_or_password`, 0x0A);
  - the status of 0x23 (`..._user_not_confirmed`, "verify your e-mail", which srp never sent)
    says a new nickname's rules: 3 or more letters, digits, dots or `- _ +`, not another
    player's.
  - The site's sign-in line says the same (`web-server` `lib.rs`).
- **A new account (first 0.26g sign-in):** the login typed is the nickname it registers with,
  and the name it is first shown by, as typed (case kept; the login is kept in lower case, as
  every login).
  - It takes a chosen name's rules (`name_refusal`, above: no `@`, a `.` taken, no blank, 3 or
    more characters, 63 bytes).
  - It is no account's login, case aside: an existing login is that account's sign-in (below).
  - Its name is no other account's name or login, case aside, Cyrillic's too (`name_taken`),
    as a rename's: a nickname another account is shown by is refused.
  - Refused: 0x23 (`NICKNAME_REFUSED`), the rules' text above. Nothing is made, nothing counts
    as a wrong password. The checks come after "registration closed", before the address's
    registration limit and the password's hash.
  - The 0.21d login server and srp's own accounts (tests, bots: `sign_in`, `sign_in_from`,
    `get_or_create_account`) keep the rule below.
- **Signing in (0.26g):** with the account's login only, case aside: an old account's (an
  e-mail, often) or the nickname a new one registered with. The owner, 2026-10-22: "the name
  the player registered with should stay their login nickname; this allows people to separate
  their ingame vs login nicknames". The name an account is shown by never signs it in; a
  rename changes that name, never the login. The password check, the lockout (per login and
  address) and the ban's reason are unchanged.
- **A new account by any other sign-in (`Logins::Any`):** the login may hold anything (no
  control characters). The name it is shown by is the login's part before its first `@`, with
  every character a chosen name may not have made `_`: `john.doe@x` shows as `john.doe`,
  `john!doe@x` as `john_doe`.
- **One name, one account:** a new account's name is no other account's name or login, case
  aside (a rename's rule). If it is, `_2`, `_3`, ... is added (the name cut so the number fits
  the 63 bytes): `john!doe@x` with a `john_doe` already there is `john_doe_2`. A name with no
  letter or digit left (a login of Chinese letters -> `___`) is `player_<id as 8 hex digits>`, as an empty one
  always was. [choice] The names are compared with no lock held: two sign-ups of one name at
  the same instant may still both get it (a new 0.26g nickname then gets its `_2`).
- **A rename is checked the same way** (`nickname_refusal`): the rules above, then
  `name_taken`, which folds every letter's case (it compared ASCII case only: `СТАЛКЕР`
  passed beside `Сталкер`). It changes the name only.
- **Finding a player by name** (friend, squad and clan requests, private lines:
  `account_named` -> `SessionStore::account_shown_as`): in every letter's case too, as names
  are unique so (`сталкер` finds `Сталкер`). Never at sign-in: a name signs no account in, so
  an account whose password the admin reset takes a new one from its login only.
- **A name saved before (on loading):** only what no name may show is made `_`: `<`, `>`,
  `&`, `"`, `'` (the end of the name's link), `:` (the link's field separator:
  `clickLinkHandler` splits on it, so the player menu would act on another name), `@`, control
  characters, and the invisible ones that turn or hide text (Unicode format characters: the
  right-to-left override, zero-width spaces and joiners, the isolates, the byte-order mark, the
  tags; and the line and paragraph separators). A name merely outside the chosen set (a
  blank) stays as players know it.
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

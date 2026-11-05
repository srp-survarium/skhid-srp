# The admin's chat mute

The rules the code and the tests follow. Code: the account's `chat_ban`, a `session::Ban` like a
sign-in ban (`Ban::chat_notice`), `SessionStore::mute_chat` / `unmute_chat` / `chat_ban`, the
lobby's `Core::chat` (`servers/lobby-server/src/v026g/social.rs`), the page's Mute chat tab
(`servers/web-server/src/moderation.rs`). Server-only: no exe, Flash or content change,
`REVISION` unchanged, no `simulation_changes` row. Not the players' own mute (`chat-mute.md`).

- **Who:** the admin, on the 0.26g admin page, tab **Mute chat**: the account (user name or
  character name), hours (empty or 0: for good; at most ten years, as a ban), and a reason. The
  reason is required: the player is told it.
- **From a script: one POST, no confirm step,** as a ban. Form fields: `token`, `tab=mute`,
  `mute_user` (user or character name), `mute_hours` (empty for good), `mute_reason`; to lift it,
  `token`, `tab=mute`, `mute_user`, `unmute=1`. From the owner's machine through the jump host,
  with `admin`/`path`/`token` read as in `password-reset.md`:

  ```bash
  name='Player'; hours=24; reason='spam'    # no " or \ in them
  printf 'url = "%s/%s"\ndata-urlencode = "token=%s"\ndata = "tab=mute"\ndata-urlencode = "mute_user=%s"\ndata-urlencode = "mute_hours=%s"\ndata-urlencode = "mute_reason=%s"\n' \
      "${SKHID_SITE%/}" "${path#/}" "$token" "$name" "$hours" "$reason" |
      ssh "$SKHID_JUMP" "curl -sS --max-time 30 -K -" | grep -o "<p class=note>[^<]*"
  ```

  To lift it, the same with `data = "unmute=1"` in place of the hours and reason lines. The answer
  is the admin page (English unless the form sends `lang=uk`); its note is one line:
  `<name>'s chat muted until <YYYY-MM-DD HH:MM> UTC.`, `<name>'s chat muted for good.`,
  `<name>'s chat unmuted.`, `<name>'s chat was not muted.`, or a refusal (`A reason is required:
  …`, `Hours: a whole number …`, `No such user.`).
- **Effect:** the muted account's chat lines reach nobody. Each line it sends is answered, to it
  alone and on the same channel, with one line said by itself: `muted for <reason>`, or
  `muted until <YYYY-MM-DD HH:MM> UTC: <reason>` for a timed mute. The game shows its own line
  locally as always (its client echoes before sending); the notice follows it.
- **Channels:** every text channel the lobby carries: general, private, squad, clan, clan team,
  match, team, and the room's chat (its line is not kept in the room's history either). The check
  is in the one place every line comes in (`4C`), before the channel is looked at.
- **The notice** goes the normal chat-line way (`A2`, the sender's own id, name and clan tag), so
  the line's length limit and the client's own handling apply.
- **Duration and expiry:** a timed mute ends by itself: it is checked when a line is sent; there is
  no timer. A new mute replaces the old one. **Unmute** (same tab, the account only) lifts it;
  one that ran out is no mute to lift.
- **Immediate:** the lobby reads the mute at each line, so it holds from the player's next line,
  in the lobby or in a match, with no reconnect.
- **Kept:** with the account in `srp.db` (`accounts.chat_ban_reason`, `chat_ban_until`: schema
  step 6), written at once, so a restart keeps it.
- **Logged:** each mute and unmute is a row of `g26_admin_actions` (kind `chat_mute`, before and
  after: `not muted`, `for good` or `until <time> UTC`, the reason) and a log line; each dropped
  line is one log line. The Bans tab lists the mutes in force.
- **Out of scope:** voice chat; friend requests and squad invites (not chat lines); the 0.21d
  chat server.
- **Not run in a game yet.**

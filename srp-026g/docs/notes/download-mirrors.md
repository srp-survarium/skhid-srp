# Download mirrors (the owner's design, 2026-11-01)

Why: Google Drive's quota for the game archive ran out on 2026-11-01; the one built-in address
answered a "Quota exceeded" page, and new players could not install. How it works in detail:
`client/updater/README.md` ("Original game download", "Patches") and the release skill
(`.claude/skills/skhid-release/SKILL.md`, "What is offered").

- Bytes pinned per release (`release/patches.tsv`, `release/game.tsv`, named in the server
  data); links dynamic: srp adds the rows of `mirrors.tsv` (`<file> <url> [<n>]`) in its data
  volume, read again when it changes. A bad row is logged and skipped, never a broken `/patches`.
- Google Drive links first, for every file (the owner's rule); the patches server's copy stays
  in every patch row (the release builds from it).
- `GET /game` beside `/patches`. `release/add-mirror` adds or removes a row at once (`--first`,
  `--at <n>`: its place among all the file's URLs).
- The updater and Skhid-without-updater.bat: addresses in order; any failure moves on; too slow
  only while another address remains; the game's archive goes on from its bytes (`Range`); one
  log line a try, a failure and the success; at a failure, where to send the log (#bugreports,
  tag install).
- Server unreachable: the addresses built in at release time. The updater and the .bat then only
  fetch missing patches and delete none; for the .bat, a server that answers but gives no list is
  a failure.
- Not done: failure reports to srp, an admin-page tab, a health-check cron.

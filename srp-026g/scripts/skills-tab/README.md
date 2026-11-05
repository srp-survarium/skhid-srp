# Skills tab

The website's `/wiki/skills`, a page of the wiki (`srp-026g/servers/web-server/src/skills.rs`):
every branch of the skill tree as the lobby draws it.

- One card per branch the tree has (the wiki's card look), named as the game names its tab.
- Each skill at its column and tier, its icon drawn as the lobby's skill renderer draws it (plate,
  branch tint, rim), with its tier numeral and one pip per rank.
- Each tier's threshold (branch points) at its left.
- Either/or skills marked "or"; hovering one shows the renderer's red lock on what it excludes,
  as the lobby does; hovering any shows its hover rim.
- A skill that needs more points than its branch can hold carries the renderer's lock.
- Hover or tap: the name, the description, each rank's value in the game's green.
- English and Ukrainian, like the rest of the wiki; the site's colours and type around the
  game's own pictures.

## Build: the release does it

`build.py` reads everything from the game as the players have it (the stock `resources.db` with
exactly the patches of `release/patches.tsv`, through `srp-026g/scripts/released-game/`):
`gameplay/skills/skills_tree.options`, the localization and the icons of `lobby_menu.swf`
(FFDec). Its docstring says what comes from where. It writes `servers/web-server/skills/`
and its `built-from.tsv`.

`release/build-site` runs it with the Maps tab and the calculator, and `release/release.sh`
rebuilds all of them when main's `built-from.tsv` names other patches than the release
offers. A web-server test fails a PR whose pages and list differ. By hand, for a preview:

    release/build-site --tree .

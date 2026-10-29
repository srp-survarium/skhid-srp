# Wiki: weapons

The wiki is `/wiki` (its index) and four sections: `/wiki/weapons`, built here;
`/wiki/modifiers`; and the damage calculator (`/wiki/calculator`,
`scripts/damage-calc/`) and the maps (`/wiki/maps`, `scripts/maps-tab/`), each a
page of its own look under a "Wiki / <page>" line. Their old addresses `/damage`
and `/maps` send to them.

The modifiers page has no build step: the web server makes it from the server's
own game data (`servers/web-server/src/modifiers.rs`), with the tables and the
roll rule the lobby gives modifications by (`Modification::rolls`). Each kind
lists its strengths, commonest first, and the item groups that roll it (or, for
the kinds that never roll, the items that come with it); a group only some of
whose items take it lists those items. Reliability is left out: items wear
nothing.

The weapons section is a searchable weapon catalogue with class filters,
sorting and expandable weapon stats, in EN/UK.
Two selected weapons share a stat comparison and recoil simulation. Comparison
colors follow each stat's direction, stay neutral for equal displayed values,
unknown values and neutral traits, and can be turned off. Range comparisons
require both endpoints to be no worse; crossed ranges stay neutral.

`stats.py` defines the common display schema for the individual cards and the
comparison. Config data includes damage, handling, dispersion, recoil and sway;
missing values are shown as absent, except for the verified native 1× hip-fire
default. Wear rates and reliability remain omitted. Horizontal recoil is the
right draw minus the left draw, never their average. Its variability is the
standard deviation of that difference for independent uniform draws.

`recoil.js` models the native shot, ramp and recovery operations at 50 ms ticks,
matching the research reference in `match-server/.../bots/recoil.rs`. Weapon
parameters all come from the dataset. The fixed animation normalization factors
63.735 / 21.48 come from the executable. Random samples use a repeatable xorshift
seed, not the native generator. This is a recoil model, not a bullet-impact
prediction: no mouse compensation, spread, camera shake or confirmed horizontal
screen projection. The UI states those limits. Next shot, reset and timeline
scrubbing update the same simulation position. Changing weapons or the shot
count resets the position; changing the random seed rebuilds both traces while
preserving the current position.

Both comparison slots use the calculator-style image chooser (`picker.js`),
with class groups, level ordering, search and keyboard navigation. The same
selection drives stats and recoil and is restored from the page URL.

The weapon names, images and numbers come from the damage calculator's generated
JSON literal in `servers/web-server/damage/calc.js`. `build.py` decodes JSON; it
does not execute JavaScript. Classes use the game's item-dictionary category,
not the weapon implementation type (several rifles use the SMG implementation).
Damage and penetration are converted to the same 100-point scale as the game UI.
Ammo, modifications and character effects are excluded explicitly on the page.

`damage-calc/build_site.py` builds this catalogue immediately after the calculator,
including during `release/build-site`. A web-server test checks the SHA-256 of
the calculator against `wiki/source.sha256`, preventing stale weapon data.

After changing the wiki template or JavaScript:

```
python3 srp-026g/scripts/wiki/build.py
```

Local browser preview (under `nix develop`, from `srp-026g`):

```
cargo test -p web-server serve_wiki_preview -- --ignored --nocapture
```

Open `http://127.0.0.1:24583/wiki`. This uses isolated in-memory accounts and
does not start or connect to the game servers. Stop the command to close it.

`check_browser.mjs` exercises the real HTTP preview in an isolated Firefox
WebDriver BiDi session (port 9457 by default), including filtering, sorting,
EN/UK, expanded rows, mobile sizing, images and the site's CSP. It saves review
screenshots to `/tmp/wiki-weapons-review`.

Focused model checks: `node scripts/wiki/test_recoil.mjs servers/web-server/damage/calc.js`
and `python3 -m unittest discover -s scripts/wiki -p 'test_*.py'` from `srp-026g`.

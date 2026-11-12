# Damage calculator (`/wiki/calculator`)

The 0.26g wiki's section "Damage calculator" / "Калькулятор шкоди": shooter against target, every
number from the game data, the rules as the client's hit code applies them. Built by
`srp-026g/scripts/damage-calc/` (its README), served by `servers/web-server/src/damage.rs`.

## Owner's rules

- The data is the game as players have it: the stock `resources.db` with every **released**
  patch over it, read by one command (`build_site.py`), never by hand. A live release rebuilds it from the
  patches it offers (`release/build-site`), never on a "don't forget" basis; its record commits it.
- English and Ukrainian, as the rest of the site (`?lang=`). Item, map and bonus names are the
  game's own localization.
- Less text: one line per rule, players know the game. No intro, no sources paragraph; the
  rules section is the formula only.
- Shooter: weapon and ammo (lists with the lobby icons; weapons by level in each class), distance 0–150 m (all fights), weapon bonuses always shown, one
  button per grade (Damage, Distance, Rate of fire, Armor penetration), no defects (negative
  bonuses). A weapon with presets (premium) has those, fixed, as the lobby lists them; damage
  and rate of fire change the numbers.
- Target: the HUD doll and the equipment cards (both aim), armour bonuses, Onyx (four slot
  tiles, passive or active), the Anesthetic, the pain already taken. The Larkspur changes no hit:
  its four tiles are in the healing section, for the recovery it gives. Under the doll: the hit
  part's pool and pain only, after the stepper's shot, tagged when each kills or breaks.
  Armour bonuses always shown, for the item on the part being hit only, under its icon and its
  armour on each part it covers (after the bonuses; the part being hit marked).
- Armour slots pick as the weapons do: the lobby icons, by level, with search.
- A closed helmet (or an exo suit) takes the slots its model covers: their cards are locked.
- Map: every map the Maps tab has (its names, its order), the game's minimap or the top-down render (a toggle, the minimap first), cropped to the play area (the native minimap's), at most 75% of the
  window's height; two clicks for the distance; the distance slider moves the target along the
  same line. On a wide screen (1700 px and up) it sits between shooter and target.
- Healing, at the bottom: recovering by itself, the Larkspur (Живокіст) carried and activated,
  the medikit and bandages, side by side; every number from the data. Under it, the time from
  empty to full for none and 1–4 Larkspurs, the picked number marked.
- Recovery bonuses (owner 2026-11-12: "it should take into account all bonuses to recovery",
  "those should all be togglable", "I want this to be combinable"): everything that changes the
  healing section's numbers is picked there, each on or off in any mix, and both tables follow:
  - the Larkspurs carried, none to four (item tiles; each adds its own regeneration callback);
  - a booster with a regeneration bonus (the Rations: speed +15 %, the delay −2 s, the limit
    +100 %), an item tile; hovering it (or focusing it; on a phone, a tap) shows its card, the
    name and its built-in bonuses as the lobby lists them (the owner: "when I hover over rations,
    I want to see what they do", "helper window");
  - the Regeneration bonus of each slot that takes one (jacket, trousers, backpack), a button per
    grade as the armour bonuses have; a piece carries one bonus of a kind;
  - the medical skills, a button per rank: Metabolism (the delay), Strong Bones (a leg's or
    hand's maximum), Medical Training and Take Cover (what a medicine heals; Take Cover's ranks
    count as crouched), Fast Bandaging (the wait before a medicine works). Each has its lobby
    icon (the skills page's, as its build draws them) and, hovering its icon or name, a card:
    its name and the lobby tooltip's line for a rank (the owner: "same for skills and maybe they
    should have skill icons too").
  The grades, the slots, the booster's bonuses and each skill's ranks and step come from the game
  data (`recovery` of `calc-data.json`; a skill's step is the number the lobby prints for a rank);
  the rules are the exe's (`rules.js`: `regeneration`, `medicine`; regeneration-booster.md,
  medical-skills.md). The picks are kept with the page's other choices.
- The map's readout sits under it, two lines whatever the state, so the map never moves.
- Compare table: every weapon with its icon, sortable by any column, column separators.
- The calculator keeps its own look; only the site's header sits above it.
- The formula says how one bullet through several hit boxes counts (each pool its largest
  share). The calculator itself shows one box per shot.
- Formula (owner 2026-11-05): numbered steps in the bullet's order (Bullet, Wall, Onyx, Armour,
  Pools, Pain, Death), each a bold label and its rule in one chip, no sentences (the trace has the
  details), the operation written out (no word standing for a value); armour and penetration in the
  page's units (x100), no letter for two things, a legend line for D, P and A. One grid for all
  rows (number | label | rule | input | D | P), legend and note inside the panel; the page's text
  font, no chips. Beside each step this shot's numbers as a running chain, from the same result
  the trace shows: the step's input (secondary), then D and P after it in their own aligned columns,
  bold (from Armour on, the hit and the pools in the D column); "none" carries the values through,
  "—" after a bullet the wall keeps. On a phone the D and P columns stay aligned under each rule.
- Wall (owner 2026-11-05): the shooter picks the wall the bullet goes through just in front of the
  target, in a drop-down: `<none>` first (the default: no change), then the material groups by
  armour, named as retail names materials where they fit (Plywood, Tinplate, Brick, steel,
  Concrete). One wall. Each group is materials of `game_materials/game.materials` with one `armor`
  (read from the data; a group whose materials differ stops the build).
- The wall's rule is the game's (`bullet::try_pierce` 0x32D960, `update_bullet_position`
  0x32EE90): k = 1 − (1 − clamp((P − A) / A, 0, 1))², 1 for A = 0; k ≤ 0 stops the bullet ("does
  not get through", one line). Else damage, penetration and speed × k, then damage and penetration
  no higher than the falloff gives at the slower speed. Every result (doll, tables, chart) is
  behind the wall; the formula section has it as one step.
- Not modelled: ricochets, the wall's deflection, explosions, anomalies.

## Serving

- `GET /wiki/calculator`: the page in the request's language, under the site's header and a "Wiki /
  Damage calculator" breadcrumb (`no-store`, as every page). Its Content-Security-Policy adds `script-src 'self'`,
  `img-src 'self' data:` and `font-src 'self'` to the site's; nothing inline runs and nothing comes
  from elsewhere.
- `GET`/`HEAD /damage/calc.js`, `/damage/fonts/*.woff2`, `/damage/maps/*.webp`: the files the
  page names with `?v=<hash>`, kept a year (`immutable`). Part of the binary.
- `GET`/`HEAD /damage` (its address before the wiki): a `301` to `/wiki/calculator?lang=<language>`.
- The 0.21d site has no calculator.

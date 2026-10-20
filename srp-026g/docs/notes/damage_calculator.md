# Damage calculator (`/damage`)

The 0.26g site's tab "Calculator" / "Калькулятор": shooter against target, every
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
- Map: every map the Maps tab has (its names, its order), the top-down render or the game's minimap (a toggle), cropped to the play area (the native minimap's), at most 75% of the
  window's height; two clicks for the distance; the distance slider moves the target along the
  same line. On a wide screen (1700 px and up) it sits between shooter and target.
- Healing, at the bottom: recovering by itself, the Larkspur (Живокіст) carried and activated,
  the medikit and bandages, side by side; every number from the data. How many Larkspurs (1–4)
  is chosen there; each adds its own regeneration callback. Under it, the time from empty to full
  for none and 1–4 Larkspurs (past two, the added wait outgrows what the speed saves).
- The map's readout sits under it, two lines whatever the state, so the map never moves.
- Compare table: every weapon with its icon, sortable by any column, column separators.
- The calculator keeps its own look; only the site's header sits above it.
- The formula says how one bullet through several hit boxes counts (each pool its largest
  share). The calculator itself shows one box per shot.
- Not modelled: cover a bullet passes through, ricochets, explosions, anomalies.

## Serving

- `GET /damage`: the page in the request's language, under the site's header (`no-store`, as
  every page). Its Content-Security-Policy adds `script-src 'self'`, `img-src 'self' data:` and
  `font-src 'self'` to the site's; nothing inline runs and nothing comes from elsewhere.
- `GET`/`HEAD /damage/calc.js`, `/damage/fonts/*.woff2`, `/damage/maps/*.webp`: the files the
  page names with `?v=<hash>`, kept a year (`immutable`). Part of the binary.
- The 0.21d site has no calculator.
